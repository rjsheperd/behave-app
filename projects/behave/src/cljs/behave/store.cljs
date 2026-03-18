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
  (atom {:sql-conn nil
         :wrapper  nil
         :db-name  nil}))

;;; SQLite Helpers

(defn- reset-sql-state! []
  (when-let [old-conn (:sql-conn @sql-state)]
    (sql/close! old-conn))
  (reset! sql-state {:sql-conn nil :wrapper nil :db-name nil}))

(defn- init-sql-conn!
  "Initialize a SQLite connection for `db-name`.
   Tries Rust restoreFromLegacy first (handles EDN .bp7 format), falls back
   to CLJS storage-async. Returns Promise of {:conn, :wrapper?, :sql-conn}."
  [schema db-name]
  (-> (sql/connect! db-name)
      (p/then (fn [sql-conn]
                ;; Try Rust restore first (handles legacy EDN format from .bp7)
                (if-let [rdb (try (.restoreFromLegacy WasmDataScript db-name)
                               (catch :default _ nil))]
                  (let [cljs-db (impl-rust/sync-from-rust rdb)]
                    {:conn     (d/conn-from-db cljs-db)
                     :sql-conn sql-conn})
                  ;; Fall back to CLJS storage-async path
                  (let [store (ds-sqlite/sqlite-store sql-conn {:db-name db-name})]
                    (-> (storage-async/restore-sync store)
                        (p/then (fn [result]
                                  (if result
                                    (let [[db wrapper] result]
                                      {:conn     (d/conn-from-db db)
                                       :wrapper  wrapper
                                       :sql-conn sql-conn})
                                    (let [wrapper (storage-async/make-sync-storage-wrapper store {})]
                                      {:conn     (d/create-conn schema {:storage wrapper})
                                       :wrapper  wrapper
                                       :sql-conn sql-conn})))))))))))

(defn- init-sql-conn-from-datoms!
  "Initialize a SQLite connection for `db-name`, create a conn from `datoms`,
   and persist to storage. Returns Promise of {:conn ds-conn, :wrapper wrapper, :sql-conn sql-conn}."
  [schema datoms db-name]
  (-> (sql/connect! db-name)
      (p/then (fn [sql-conn]
                (let [store   (ds-sqlite/sqlite-store sql-conn {:db-name db-name})
                      wrapper (storage-async/make-sync-storage-wrapper store {})
                      ds-conn (d/conn-from-datoms datoms schema {:storage wrapper})]
                  (-> (storage-async/store-impl-sync! (d/db ds-conn) wrapper true)
                      (p/then (fn [_]
                                {:conn     ds-conn
                                 :wrapper  wrapper
                                 :sql-conn sql-conn}))))))))

(defn- read-file-bytes
  "Read a File object as Uint8Array. Returns a Promise."
  [file]
  (js/Promise.
   (fn [res _reject]
     (let [rdr (js/FileReader.)]
       (set! (.-onload rdr) (fn [_] (res (js/Uint8Array. (.-result rdr)))))
       (.readAsArrayBuffer rdr file)))))

(defn- import-sqlite-bytes!
  "Import raw SQLite bytes into a new database. Returns Promise of
   {:conn ds-conn, :wrapper?, :sql-conn sql-conn}.
   Tries Rust restoreFromLegacy first, falls back to CLJS storage-async."
  [db-bytes db-name]
  (-> (sql/connect! db-name)
      (p/then (fn [tmp-conn]
                (-> (sql/import! tmp-conn db-bytes)
                    (p/then (fn [_] (sql/close! tmp-conn))))))
      (p/then (fn [_] (sql/connect! db-name)))
      (p/then (fn [sql-conn]
                ;; Try Rust restore first
                (if-let [rdb (try (.restoreFromLegacy WasmDataScript db-name)
                               (catch :default _ nil))]
                  (let [cljs-db (impl-rust/sync-from-rust rdb)]
                    {:conn     (d/conn-from-db cljs-db)
                     :sql-conn sql-conn})
                  ;; Legacy fallback for older .bp7 files
                  (let [store (ds-sqlite/sqlite-store sql-conn {:db-name  db-name
                                                                :skip-ddl true})]
                    (-> (storage-async/restore-sync store)
                        (p/then (fn [[db wrapper]]
                                  {:conn     (d/conn-from-db db)
                                   :wrapper  wrapper
                                   :sql-conn sql-conn})))))))))

(defn- new-worksheet-tx [ds-conn ws-name ws-uuid modules]
  (let [version @(rf/subscribe [:state :app-version])
        tx (cond-> {:worksheet/uuid    ws-uuid
                    :worksheet/modules modules
                    :worksheet/created (.now js/Date)}

             version
             (assoc :worksheet/version version)

             ws-name
             (assoc :worksheet/name ws-name))]
    (d/transact ds-conn [tx])))

;;; Conn Initialization

(defn- setup-conn!
  "Wire up a DataScript conn: set the conn atom, sync to Rust worksheet DB,
   register Rust sync listener, then connect re-posh and reactive-entity.
   Rust sync must happen BEFORE re-posh so posh reactive queries use
   the updated Rust DB (listeners fire in registration order)."
  [ds-conn]
  (reset! conn ds-conn)
  ;; Sync worksheet DB to Rust BEFORE connecting re-posh
  (let [rdb (impl-rust/sync-to-rust! @ds-conn)]
    (impl-rust/set-named-db! "$ws" rdb @ds-conn))
  ;; Register Rust sync listener BEFORE re-posh's listener
  (d/listen! ds-conn :rust-sync
    (fn [{:keys [db-after]}]
      ;; Full re-sync on each tx (worksheet DBs are small, <1K datoms)
      (let [rdb (impl-rust/sync-to-rust! db-after)]
        (impl-rust/set-named-db! "$ws" rdb db-after))))
  ;; Now connect re-posh (registers listener after ours)
  (rp/connect! ds-conn)
  (re/init! ds-conn)
  ds-conn)

(defn- reset-conn-state! []
  (when @conn
    (d/unlisten! @conn :rust-sync))
  (impl-rust/remove-named-db! "$ws")
  (reset! conn nil)
  (reset! worksheet-from-file? false))

;;; SQLite Sync (load initial state from AbsurderSQL/IndexedDB)

(defn load-store-minimal!
  "Set up an in-memory DataScript conn without SQLite backing.
   Used on initial load when no worksheet is active.
   Awaits WASM init but does not return a Promise to the caller."
  []
  (-> (pss/ensure-initialized!)
      (p/then (fn [_]
                (let [schema (->ds-schema all-schemas)]
                  (setup-conn! (d/create-conn schema))
                  (rf/dispatch-sync [:state/set :sync-loaded? true])))))
  nil)

(defn load-store-local!
  "Initialize a local DataScript connection backed by SQLite.
   Attempts to restore an existing DB named `worksheet-<ws-uuid>.db`."
  [ws-uuid]
  (let [schema  (->ds-schema all-schemas)
        db-name (str "worksheet-" ws-uuid ".db")]
    (-> (p/all [(pss/ensure-initialized!) (sql/init!)])
        (p/then (fn [_] (init-sql-conn! schema db-name)))
        (p/then (fn [{ds-conn :conn :keys [wrapper sql-conn]}]
                  (swap! sql-state assoc
                         :sql-conn sql-conn
                         :wrapper  wrapper
                         :db-name  db-name)
                  (setup-conn! ds-conn)
                  (rf/dispatch-sync [:state/set :sync-loaded? true])))
        (p/catch (fn [e]
                   (js/console.error "Failed to initialize local store:" e)
                   (setup-conn! (d/create-conn schema))
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
  "Persist DataScript to SQLite and export as bytes. Returns a Promise of Uint8Array.
   Uses Rust storeToLegacy when available, falls back to CLJS storage-async."
  []
  (let [{:keys [sql-conn db-name wrapper]} @sql-state]
    (when sql-conn
      (if-let [rdb (impl-rust/named-db "$ws")]
        ;; Rust persistence: storeToLegacy writes EDN to datascript table
        (do (.storeToLegacy rdb db-name)
            (sql/export! sql-conn))
        ;; Legacy fallback (needs wrapper)
        (when wrapper
          (-> (storage-async/store-impl-sync! (d/db @conn) wrapper true)
              (p/then (fn [_] (sql/export! sql-conn)))))))))

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
                  (import-sqlite-bytes! db-bytes db-name)))
        (p/then (fn [{ds-conn :conn :keys [wrapper sql-conn]}]
                  (swap! sql-state assoc
                         :sql-conn sql-conn
                         :wrapper  wrapper
                         :db-name  db-name)
                  (setup-conn! ds-conn)
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
    (rf/dispatch-sync [:state/set :sync-loaded? false])
    (reset-conn-state!)
    (reset-sql-state!)
    (reset! worksheet-from-file? false)
    (-> (sql/init!)
        (p/then (fn [_] (init-sql-conn! schema db-name)))
        (p/then (fn [{ds-conn :conn :keys [wrapper sql-conn]}]
                  (swap! sql-state assoc
                         :sql-conn sql-conn
                         :wrapper  wrapper
                         :db-name  db-name)
                  (setup-conn! ds-conn)
                  (new-worksheet-tx ds-conn ws-name ws-uuid modules)
                  (reset! current-route-order
                          @(rf/subscribe [:wizard/route-order ws-uuid workflow]))
                  (rf/dispatch-sync [:state/set :sync-loaded? true])
                  (rf/dispatch-sync [:navigate (first @current-route-order)])))
        (p/catch (fn [e]
                   (js/console.error "New worksheet failed:" e))))))

;;; Public Fns

(defn init! [{:keys [datoms schema]}]
  (if @conn
    @conn
    (do
      (setup-conn! (d/conn-from-datoms datoms schema))
      @conn)))

;;; Effects

(rf/reg-fx :ds/init init!)

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
