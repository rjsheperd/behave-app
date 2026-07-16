(ns behave.store
  "Worksheet store. The Rust `WasmDataScript` instance is the source of truth;
   a lightweight CLJS conn mirrors it for posh/reactive-entity reactivity.

   Persistence is owned by the Rust engine (see SIMPLIFY_RUST_DS.org Phase A):
   `WasmDataScript.open` opens the AbsurderSQL database (IndexedDB VFS) and
   restores any stored worksheet; `.persist` stores and flushes to IndexedDB
   in one call. There is no connect-first contract — a missing connection is
   a loud error, never a silent in-memory write."
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core           :as d]
            [absurder-sql.datascript.impl-rust      :as impl-rust]
            [absurder-sql.datascript.persistent-sorted-set :as pss]
            [austinbirch.reactive-entity            :as re]
            [behave-routing.main                    :refer [current-route-order]]
            [behave.schema.core                     :refer [all-schemas]]
            [browser-utils.core                     :refer [download]]
            [ds-schema-utils.interface              :refer [->ds-schema]]
            [promesa.core                           :as p]
            [re-frame.core                          :as rf]
            [re-posh.core                           :as rp]))

;;; State

(defonce conn (atom nil))
(defonce ^:private worksheet-from-file? (atom false))

;; AbsurderSQL db name for the active worksheet (used by the debounced
;; persist and by export). Non-private so integration tests can assert on it.
(defonce active-db-name (atom nil))

;;; Persistence

(defn- persist!
  "Store the Rust DB to its AbsurderSQL SQLite and flush to IndexedDB.
   Returns a Promise. Fails loud — errors are logged and re-thrown to the
   global window.onerror handler (behave.telemetry) rather than swallowed."
  [rdb db-name]
  (-> (p/do (.persist rdb db-name))
      (p/catch (fn [e]
                 (js/console.error "[store] Rust persist failed for" db-name e)
                 (throw e)))))

(defonce persist-timer (atom nil))

(defn- schedule-persist!
  "Debounced persist: flush the Rust DB to IndexedDB 2s after the last edit."
  []
  (when-let [t @persist-timer] (js/clearTimeout t))
  (reset! persist-timer
          (js/setTimeout
           (fn []
             (let [db-name @active-db-name
                   rdb     (impl-rust/named-db "$ws")]
               (when (and rdb db-name)
                 (persist! rdb db-name))))
           2000)))

;;; Helpers

(defn- reset-persist-state! []
  (when-let [t @persist-timer] (js/clearTimeout t))
  (reset! persist-timer nil)
  (reset! active-db-name nil))

(defn- read-file-bytes
  "Read a File object as Uint8Array. Returns a Promise."
  [file]
  (js/Promise.
   (fn [res _reject]
     (let [rdr (js/FileReader.)]
       (set! (.-onload rdr) (fn [_] (res (js/Uint8Array. (.-result rdr)))))
       (.readAsArrayBuffer rdr file)))))

(defn- worksheet-uuid
  "The `:worksheet/uuid` in a CLJS db value, or nil."
  [cljs-db]
  (d/q '[:find ?uuid . :where [_ :worksheet/uuid ?uuid]] cljs-db))

;;; Conn Initialization (Rust-primary, CLJS for reactivity only)

(defn- setup-worksheet-conn!
  "Create a lightweight CLJS conn for posh reactivity (no storage adapter).
   Rust WasmDataScript is the source of truth for worksheet data."
  [schema datoms]
  (let [ds-conn (if (seq datoms)
                  (d/conn-from-datoms datoms schema)
                  (d/create-conn schema))]
    (reset! conn ds-conn)
    (rp/connect! ds-conn)
    (re/init! ds-conn)
    ds-conn))

(defn- mirror-rust-db!
  "Mirror the Rust db into a fresh CLJS conn and register it as `$ws`.
   Aligns the mirror's max-eid with the Rust allocator — conn-from-datoms
   recomputes max-eid from live datoms, which diverges from Rust's persisted
   high-water mark after any retraction (see impl-rust/sync-from-rust)."
  [schema rdb]
  (let [cljs-db (impl-rust/sync-from-rust rdb)
        ds-conn (setup-worksheet-conn! schema (d/datoms cljs-db :eavt))]
    (swap! ds-conn assoc :max-eid (:max-eid cljs-db))
    (impl-rust/set-named-db! "$ws" rdb @ds-conn)
    ds-conn))

(defn- reset-conn-state! []
  (impl-rust/remove-named-db! "$ws")
  (reset! conn nil)
  (reset! worksheet-from-file? false))

(defn- close-previous-worksheet!
  "Flush and close the active worksheet's AbsurderSQL database, unless it is
   `keep-db-name` (the worksheet being switched to). Frees the Rust
   WasmDataScript so its index storages release their pooled-connection refs,
   and closes the database (releasing the leader-election heartbeat). Without
   this, every worksheet switch leaks a Database, a SQLite connection, and a
   heartbeat interval. Must run before `reset-conn-state!` (which drops the
   `$ws` handle); async, and runs concurrently with opening the next
   worksheet (a different db, so no contention)."
  [keep-db-name]
  (let [db-name @active-db-name
        rdb     (impl-rust/named-db "$ws")]
    (when (and db-name (not= db-name keep-db-name))
      (-> (if rdb (persist! rdb db-name) (p/resolved nil))
          (p/then (fn [_] (.closeDb WasmDataScript db-name)))
          (p/then (fn [_] (when rdb (.free rdb))))
          (p/catch (fn [e]
                     (js/console.error "[store] failed closing previous worksheet"
                                       db-name e)))))
    nil))

(defn- rekey-to-canonical!
  "A worksheet opened from a file is imported under its file name. Re-key
   persistence to the canonical `worksheet-<uuid>.db` so uuid-based reload
   (`load-store-local!`) finds it and user file names can't collide.
   Returns a Promise. No-op when the worksheet has no uuid."
  [rdb cljs-db]
  (if-let [ws-uuid (worksheet-uuid cljs-db)]
    (let [canonical (str "worksheet-" ws-uuid ".db")]
      (-> (p/do (.ensureDb WasmDataScript canonical))
          (p/then (fn [_] (persist! rdb canonical)))
          (p/then (fn [_] (reset! active-db-name canonical)))))
    (p/resolved nil)))

;;; Store Initialization

(defn load-store-minimal!
  "Set up an in-memory DataScript conn without SQLite backing.
   Used on initial load when no worksheet is active."
  []
  (close-previous-worksheet! nil)
  (reset-conn-state!)
  (reset-persist-state!)
  (-> (pss/ensure-initialized!)
      (p/then (fn [_]
                (setup-worksheet-conn! (->ds-schema all-schemas) nil)
                (rf/dispatch-sync [:state/set :sync-loaded? true]))))
  nil)

(defn load-store-local!
  "Open the worksheet's AbsurderSQL database (`worksheet-<ws-uuid>.db`) and
   restore whatever is stored in it — a fresh empty db when nothing is —
   then mirror it into a CLJS conn for reactivity."
  [ws-uuid]
  (let [schema  (->ds-schema all-schemas)
        db-name (str "worksheet-" ws-uuid ".db")]
    (close-previous-worksheet! db-name)
    (reset-conn-state!)
    (reset-persist-state!)
    ;; Track the target worksheet synchronously so active-db-name always
    ;; reflects the current worksheet even while the async open is in flight.
    ;; Safe: the persist timer was just cancelled and $ws is nil until the
    ;; open completes, so no persist can fire against a wrong/incomplete db.
    (reset! active-db-name db-name)
    (-> (pss/ensure-initialized!)
        (p/then (fn [_] (.open WasmDataScript db-name (impl-rust/schema->js schema))))
        (p/then (fn [rdb]
                  (mirror-rust-db! schema rdb)
                  (rf/dispatch-sync [:state/set :sync-loaded? true])))
        (p/catch (fn [e]
                   (js/console.error "Failed to initialize local store:" e)
                   (setup-worksheet-conn! schema nil)
                   (rf/dispatch-sync [:state/set :sync-loaded? true]))))))

;;; Save Worksheet (export .bp7)

(defn- uint8-array->base64
  "Encode a Uint8Array to a base64 string in chunks to avoid stack overflow."
  [^js uint8arr]
  (let [len    (.-length uint8arr)
        chunks (array)]
    (loop [i 0]
      (when (< i len)
        (.push chunks (.apply (.-fromCharCode js/String)
                              nil
                              (.subarray uint8arr i (min (+ i 8192) len))))
        (recur (+ i 8192))))
    (js/btoa (.join chunks ""))))

(defn- export-db-bytes!
  "Persist the Rust DB then export it as SQLite bytes (.bp7).
   Returns a Promise of Uint8Array."
  []
  (when-let [db-name @active-db-name]
    (when-let [rdb (impl-rust/named-db "$ws")]
      (p/do (.exportDb rdb db-name)))))

(defn- save-worksheet-browser!
  "Save worksheet via browser blob download."
  [file-name db-bytes]
  (download db-bytes file-name "application/x-sqlite3"))

(defn- save-worksheet-cef!
  "Save worksheet via cefQuery message to JCEF backend for native save dialog."
  [file-name db-bytes]
  (let [b64 (uint8-array->base64 db-bytes)]
    (js/window.cefQuery
     #js {:request   (str "save-file:" file-name "|" b64)
          :onSuccess (fn [response] (js/console.log "Save result:" response))
          :onFailure (fn [error-code error-message]
                       (js/console.error "Save failed:" error-code error-message))})))

(defn save-worksheet! [{:keys [file-name jar-local?]}]
  (-> (export-db-bytes!)
      (p/then (fn [db-bytes]
                (if jar-local?
                  (save-worksheet-cef! file-name db-bytes)
                  (save-worksheet-browser! file-name db-bytes))))
      (p/catch (fn [e]
                 (js/console.error "Save failed:" e)))))

;;; Open Worksheet (import .bp7)

(defn open-worksheet! [{:keys [file]}]
  (let [db-name (.-name file)
        schema  (->ds-schema all-schemas)]
    (close-previous-worksheet! db-name)
    (reset-conn-state!)
    (reset-persist-state!)
    (reset! worksheet-from-file? true)
    (-> (pss/ensure-initialized!)
        (p/then (fn [_] (read-file-bytes file)))
        (p/then (fn [db-bytes] (.importDb WasmDataScript db-name db-bytes)))
        (p/then (fn [_] (.open WasmDataScript db-name (impl-rust/schema->js schema))))
        (p/then (fn [rdb]
                  (if (pos? (.count rdb))
                    (let [ds-conn (mirror-rust-db! schema rdb)]
                      (reset! active-db-name db-name)
                      (rekey-to-canonical! rdb (d/db ds-conn)))
                    (js/console.error "Unsupported .bp7 format: no worksheet data in" db-name))))
        (p/then (fn [_]
                  (rf/dispatch-sync [:state/set :sync-loaded? true])
                  (rf/dispatch-sync [:state/set :ws-version
                                     @(rf/subscribe [:worksheet/version
                                                     @(rf/subscribe [:worksheet/latest])])])))
        (p/catch (fn [e]
                   (js/console.error "Open worksheet failed:" e))))))

;;; New Worksheet

(defn new-worksheet! [ws-name modules _submodule workflow]
  (let [schema  (->ds-schema all-schemas)
        ws-uuid (str (d/squuid))
        db-name (str "worksheet-" ws-uuid ".db")]
    ;; Safe inside an event handler (these mutations don't dispatch).
    (close-previous-worksheet! db-name)
    (reset-conn-state!)
    (reset-persist-state!)
    (-> (pss/ensure-initialized!)
        ;; Fresh uuid — open returns an empty Rust db bound to its SQLite.
        (p/then (fn [_] (.open WasmDataScript db-name (impl-rust/schema->js schema))))
        (p/then (fn [rdb]
                  (let [version @(rf/subscribe [:state :app-version])
                        tx      (cond-> {:worksheet/uuid    ws-uuid
                                         :worksheet/modules modules
                                         :worksheet/created (.now js/Date)}
                                  version (assoc :worksheet/version version)
                                  ws-name (assoc :worksheet/name ws-name))
                        reveal! (fn []
                                  (rf/dispatch-sync [:state/set :sync-loaded? true])
                                  (rf/dispatch [:navigate (first @current-route-order)]))]
                    (.transact rdb (pr-str [tx]))
                    (mirror-rust-db! schema rdb)
                    (reset! active-db-name db-name)
                    (reset! current-route-order
                            @(rf/subscribe [:wizard/route-order ws-uuid workflow]))
                    ;; Persist, THEN reveal + navigate — so a refresh can
                    ;; restore it. On persist failure, still reveal the
                    ;; in-memory worksheet.
                    (-> (persist! rdb db-name)
                        (p/then (fn [_] (reveal!)))
                        (p/catch (fn [e]
                                   (js/console.error "[store] new-worksheet persist failed:" e)
                                   (reveal!)))))))
        (p/catch (fn [e]
                   (js/console.error "[store] new-worksheet failed:" e))))))

;;; Public Fns

(defn init! [{:keys [datoms schema]}]
  (if @conn
    @conn
    (do
      (setup-worksheet-conn! schema (seq datoms))
      @conn)))

;;; Effects

(rf/reg-fx :ds/init init!)

;; Override re-posh's :transact effect to use Rust as the primary engine:
;; transact through Rust first (fast, in-place), then update the CLJS conn for
;; posh reactivity, then schedule a debounced persist to IndexedDB.
(rf/reg-fx :transact
           (fn [tx-data]
             (if-let [rdb (impl-rust/named-db "$ws")]
               (do
                 ;; Rust is the source of truth. Fail loud on a Rust write
                 ;; error rather than masking it — swallowing here would
                 ;; silently diverge the Rust DB from the CLJS reactivity
                 ;; mirror. NOTE: .transact reports failures via an `error`
                 ;; key on the returned object, not by throwing.
                 (let [report (try
                                (.transact rdb (pr-str (vec tx-data)))
                                (catch :default e
                                  (js/console.error "[store] Rust transact failed for"
                                                    (vec tx-data) e)
                                  (throw e)))]
                   (when-let [err (some-> report (aget "error"))]
                     (js/console.error "[store] Rust transact error for"
                                       (vec tx-data) err)
                     (throw (js/Error. (str "Rust transact error: " err)))))
                 (d/transact! @conn tx-data)
                 (impl-rust/set-named-db! "$ws" rdb (d/db @conn))
                 (schedule-persist!))
               ;; No Rust DB — fall back to CLJS-only transact.
               (when @conn
                 (d/transact! @conn tx-data)))))

;;; Events

(rf/reg-event-fx
 :ds/initialize
 (fn [_ [_ schema datoms]]
   {:ds/init {:datoms datoms :schema schema}}))

(rp/reg-event-ds
 :ds/transact
 (fn [_ [_ tx-data]]
   (first tx-data)))

(rp/reg-event-ds
 :ds/transact-many
 (fn [_ [_ tx-data]]
   tx-data))
