(ns behave.store
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [ajax.core                              :refer [ajax-request]]
            [ajax.edn                                :refer [edn-request-format]]
            [ajax.protocols                         :as pr]
            [austinbirch.reactive-entity            :as re]
            [behave-routing.main                    :refer [current-route-order]]
            [behave.schema.core                     :refer [all-schemas]]
            [browser-utils.core                     :refer [download]]
            [absurder-sql.datascript.core           :as d]
            [absurder-sql.datascript.impl-rust      :as impl-rust]
            [absurder-sql.datascript.persistent-sorted-set :as pss]
            [absurder-sql.datascript.sqlite         :as ds-sqlite]
            [absurder-sql.datascript.storage-async  :as storage-async]
            [absurder-sql.interface                 :as sql]
            [ds-schema-utils.interface              :refer [->ds-schema]]
            [re-frame.core                          :as rf]
            [re-posh.core                           :as rp]
            [promesa.core                           :as p]))

;;; State

(defonce conn (atom nil))
(defonce ^:private worksheet-from-file? (atom false))

(defonce ^:private sql-state
  (atom {:db-name nil}))

;;; Debounced persistence

(defonce ^:private persist-timer (atom nil))

(defn- schedule-persist!
  "Debounced SQLite persist. Writes Rust DB to SQLite via storeToLegacy
   after 2s of inactivity."
  []
  (when-let [t @persist-timer] (js/clearTimeout t))
  (reset! persist-timer
    (js/setTimeout
      (fn []
        (when-let [rdb (impl-rust/named-db "$ws")]
          (when-let [db-name (:db-name @sql-state)]
            (.storeToLegacy rdb db-name))))
      2000)))

;;; SQLite Helpers

(defn- reset-sql-state! []
  (when-let [t @persist-timer] (js/clearTimeout t))
  (reset! persist-timer nil)
  (reset! sql-state {:db-name nil}))

(defn- read-file-bytes
  "Read a File object as Uint8Array. Returns a Promise."
  [file]
  (js/Promise.
   (fn [res _reject]
     (let [rdr (js/FileReader.)]
       (set! (.-onload rdr) (fn [_] (res (js/Uint8Array. (.-result rdr)))))
       (.readAsArrayBuffer rdr file)))))

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

(defn- reset-conn-state! []
  (impl-rust/remove-named-db! "$ws")
  (reset! conn nil)
  (reset! worksheet-from-file? false))

;;; SQLite Sync (load initial state from AbsurderSQL/IndexedDB)

(defn load-store-minimal!
  "Set up an in-memory DataScript conn without SQLite backing.
   Used on initial load when no worksheet is active.
   Awaits WASM init but does not return a Promise to the caller."
  []
  (reset-conn-state!)
  (-> (pss/ensure-initialized!)
      (p/then (fn [_]
                (let [schema (->ds-schema all-schemas)]
                  (setup-worksheet-conn! schema nil)
                  (rf/dispatch-sync [:state/set :sync-loaded? true])))))
  nil)

(defn load-store-local!
  "Initialize a local DataScript connection backed by Rust DB.
   Attempts to restore an existing DB named `worksheet-<ws-uuid>.db`."
  [ws-uuid]
  (reset-conn-state!)
  (let [schema  (->ds-schema all-schemas)
        db-name (str "worksheet-" ws-uuid ".db")]
    (swap! sql-state assoc :db-name db-name)
    (-> (pss/ensure-initialized!)
        (p/then (fn [_]
                  (if-let [rdb (try (.restoreFromLegacy WasmDataScript db-name)
                                 (catch :default _ nil))]
                    (let [cljs-db (impl-rust/sync-from-rust rdb)
                          ds-conn (setup-worksheet-conn! schema (d/datoms cljs-db :eavt))]
                      (impl-rust/set-named-db! "$ws" rdb @ds-conn)
                      (rf/dispatch-sync [:state/set :sync-loaded? true]))
                    ;; No existing DB — empty worksheet
                    (do (setup-worksheet-conn! schema nil)
                        (rf/dispatch-sync [:state/set :sync-loaded? true])))))
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
  "Persist Rust DB to SQLite and export as bytes. Returns a Promise of Uint8Array.
   Uses sql/init! + sql/connect! here since this is user-initiated (app fully loaded)."
  []
  (when-let [db-name (:db-name @sql-state)]
    (when-let [rdb (impl-rust/named-db "$ws")]
      (.storeToLegacy rdb db-name)
      (-> (sql/init!)
          (p/then (fn [_] (sql/connect! db-name)))
          (p/then (fn [sql-conn] (sql/export! sql-conn)))))))

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
  (let [db-name (.-name file)]
    (reset-conn-state!)
    (reset-sql-state!)
    (reset! worksheet-from-file? true)
    (-> (sql/init!)
        (p/then (fn [_] (read-file-bytes file)))
        (p/then (fn [db-bytes]
                  (-> (sql/connect! db-name)
                      (p/then (fn [tmp-conn]
                                (-> (sql/import! tmp-conn db-bytes)
                                    (p/then (fn [_] (sql/close! tmp-conn))))))
                      (p/then (fn [_] (sql/connect! db-name)))
                      (p/then (fn [sql-conn]
                                (swap! sql-state assoc :db-name db-name)
                                (let [schema (->ds-schema all-schemas)
                                      rdb    (try (.restoreFromLegacy WasmDataScript db-name)
                                               (catch :default _ nil))]
                                  (if rdb
                                    (let [cljs-db (impl-rust/sync-from-rust rdb)
                                          ds-conn (setup-worksheet-conn! schema (d/datoms cljs-db :eavt))]
                                      (impl-rust/set-named-db! "$ws" rdb @ds-conn))
                                    ;; Fallback: CLJS storage-async for very old formats
                                    (let [store (ds-sqlite/sqlite-store sql-conn {:db-name db-name :skip-ddl true})]
                                      (-> (storage-async/restore-sync store)
                                          (p/then (fn [[db _wrapper]]
                                                    (let [ds-conn (setup-worksheet-conn! schema (d/datoms db :eavt))
                                                          rdb (impl-rust/sync-to-rust! @ds-conn)]
                                                      (impl-rust/set-named-db! "$ws" rdb @ds-conn))))))))
                                (rf/dispatch-sync [:state/set :sync-loaded? true])
                                (rf/dispatch-sync [:state/set :ws-version
                                                   @(rf/subscribe [:worksheet/version
                                                                   @(rf/subscribe [:worksheet/latest])])]))))))
        (p/catch (fn [e]
                   (js/console.error "Open worksheet failed:" e))))))

;;; New Worksheet

(defn new-worksheet! [ws-name modules _submodule workflow]
  (let [schema    (->ds-schema all-schemas)
        ws-uuid   (str (d/squuid))
        db-name   (str "worksheet-" ws-uuid ".db")
        js-schema (impl-rust/schema->js schema)]
    ;; These mutations are safe inside an event handler (they don't dispatch)
    (reset-conn-state!)
    (reset-sql-state!)
    (reset! worksheet-from-file? false)
    ;; 1. Create Rust DB and transact worksheet entity synchronously
    (let [rdb     (.emptyDb WasmDataScript js-schema)
          version @(rf/subscribe [:state :app-version])
          tx      (cond-> {:worksheet/uuid    ws-uuid
                           :worksheet/modules modules
                           :worksheet/created (.now js/Date)}
                    version  (assoc :worksheet/version version)
                    ws-name  (assoc :worksheet/name ws-name))]
      (.transact rdb (pr-str [tx]))
      ;; 2. Sync Rust→CLJS for posh reactivity (no storage adapter)
      (let [cljs-db (impl-rust/sync-from-rust rdb)
            ds-conn (setup-worksheet-conn! schema (d/datoms cljs-db :eavt))]
        (impl-rust/set-named-db! "$ws" rdb @ds-conn)
        ;; 3. Persist to SQLite via storeToLegacy (no separate sql/init! needed)
        (swap! sql-state assoc :db-name db-name)
        (.storeToLegacy rdb db-name)
        (reset! current-route-order
                @(rf/subscribe [:wizard/route-order ws-uuid workflow]))
        ;; Defer dispatches — we're inside an event handler, can't dispatch-sync
        (js/setTimeout
          (fn []
            (rf/dispatch-sync [:state/set :sync-loaded? true])
            (rf/dispatch [:navigate (first @current-route-order)]))
          0)))))

;;; Public Fns

(defn init! [{:keys [datoms schema]}]
  (if @conn
    @conn
    (do
      (setup-worksheet-conn! schema (seq datoms))
      @conn)))

;;; Effects

(rf/reg-fx :ds/init init!)

;; Override re-posh's :transact effect to use Rust as primary engine.
;; Transact through Rust first (fast, in-place), then update CLJS conn
;; for posh reactivity. No storage adapter — persistence is debounced.
(rf/reg-fx :transact
  (fn [tx-data]
    (if-let [rdb (impl-rust/named-db "$ws")]
      (do
        (.transact rdb (pr-str (vec tx-data)))
        (d/transact! @conn tx-data)
        (impl-rust/set-named-db! "$ws" rdb (d/db @conn))
        (schedule-persist!))
      ;; No Rust DB — fall back to CLJS-only transact
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
