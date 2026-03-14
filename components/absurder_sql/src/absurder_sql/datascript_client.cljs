(ns absurder-sql.datascript-client
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.db :as db]
            [absurder-sql.datascript.persistent-sorted-set :as pss]
            [absurder-sql.interface :as sql]
            [cljs.reader :as reader]
            [promesa.core :as p]))

;;; State

(defonce ^:private state
  (atom {:conn     nil
         :sql-conn nil
         :rust-db  nil
         :db-name  "datascript-test.db"}))

(def ^:private demo-schema
  {:aka {:db/cardinality :db.cardinality/many}})

;;; Schema conversion (CLJS <-> JS for WasmDataScript)

(defn- schema->js
  "Convert a CLJS DataScript schema map to the JS object format WasmDataScript expects."
  [schema]
  (let [obj (js/Object.)]
    (doseq [[attr-kw props] schema]
      (let [key (str ":" (if (namespace attr-kw)
                           (str (namespace attr-kw) "/" (name attr-kw))
                           (name attr-kw)))
            p   (js/Object.)]
        (when (:db/index props)
          (unchecked-set p ":db/index" true))
        (when-let [u (:db/unique props)]
          (unchecked-set p ":db/unique" (str u)))
        (when (= :db.cardinality/many (:db/cardinality props))
          (unchecked-set p ":db/cardinality" ":db.cardinality/many"))
        (when (= :db.type/ref (:db/valueType props))
          (unchecked-set p ":db/valueType" ":db.type/ref"))
        (when (:db/isComponent props)
          (unchecked-set p ":db/isComponent" true))
        (unchecked-set obj key p)))
    obj))

(defn- js->schema
  "Convert a JS schema object (from WasmDataScript) back to a CLJS schema map."
  [js-schema]
  (when (and js-schema (not (undefined? js-schema)))
    (let [entries (js/Object.entries js-schema)]
      (into {}
            (map (fn [entry]
                   (let [k     (aget entry 0)
                         v     (aget entry 1)
                         kw    (keyword (subs k 1))
                         props (cond-> {}
                                 (aget v ":db/index")
                                 (assoc :db/index true)

                                 (aget v ":db/unique")
                                 (assoc :db/unique (keyword (subs (aget v ":db/unique") 1)))

                                 (aget v ":db/cardinality")
                                 (assoc :db/cardinality (keyword (subs (aget v ":db/cardinality") 1)))

                                 (aget v ":db/valueType")
                                 (assoc :db/valueType (keyword (subs (aget v ":db/valueType") 1)))

                                 (aget v ":db/isComponent")
                                 (assoc :db/isComponent true))]
                     [kw props])))
            (array-seq entries)))))

;;; CLJS DB <-> WasmDataScript bridge

(defn- keyword->attr-str
  "Convert a CLJS keyword to the string format WasmDataScript expects.
   :person/name -> \":person/name\", :name -> \":name\"."
  [kw]
  (if (namespace kw)
    (str ":" (namespace kw) "/" (name kw))
    (str ":" (name kw))))

(defn- js-datom-attr->keyword
  "Convert a JS datom attribute string like ':name' or ':ns/name' to a keyword."
  [^string s]
  (keyword (subs s 1)))

(defn- cljs-db->rust-db!
  "Sync the current CLJS DataScript DB to a WasmDataScript instance.
   Extracts all datoms and rebuilds the Rust DB from scratch."
  [cljs-db]
  (let [schema   (:schema cljs-db)
        js-schema (schema->js schema)
        datoms   (d/datoms cljs-db :eavt)
        rust-db  (.emptyDb WasmDataScript js-schema)
        arr      (js/Array.)]
    (doseq [^db/Datom d datoms]
      (.push arr #js {:e  (.-e d)
                      :a  (keyword->attr-str (.-a d))
                      :v  (.-v d)
                      :tx (.-tx d)}))
    (.withDatoms rust-db arr)))

(defn- rust-db->cljs-db
  "Create a CLJS DataScript DB from a WasmDataScript instance.
   Extracts all datoms from the Rust DB and creates a CLJS DB via init-db."
  [rust-db]
  (let [js-schema (.schema rust-db)
        schema    (js->schema js-schema)
        datoms-arr (.datomsIndex rust-db "eavt"
                                 js/undefined js/undefined js/undefined js/undefined
                                 js/undefined js/undefined js/undefined js/undefined)
        datoms    (into []
                        (map (fn [d]
                               (db/datom (.-e d)
                                         (js-datom-attr->keyword (.-a d))
                                         (.-v d)
                                         (.-tx d))))
                        (array-seq datoms-arr))]
    (d/init-db datoms schema)))

;;; UI Helpers

(defn- log! [msg]
  (let [el (.getElementById js/document "log")]
    (set! (.-textContent el)
          (str (.-textContent el) msg "\n"))
    (set! (.-scrollTop el) (.-scrollHeight el))))

(defn- set-status! [s]
  (set! (.-textContent (.getElementById js/document "status"))
        (str "Status: " s)))

(defn- render-results! [results]
  (let [el (.getElementById js/document "results")]
    (set! (.-textContent el) (pr-str results))))

;;; Legacy restore (Rust-native EDN parsing from `datascript` table)

(defn- legacy-restore
  "Restore from legacy EDN format (`.bp7` files) via Rust `restoreFromLegacy`.
   The `datascript` table must exist with EDN metadata at addr=0.
   Returns {:conn conn :rust-db rust-db} or nil."
  [db-name]
  (try
    (when-let [rust-db (.restoreFromLegacy WasmDataScript db-name)]
      (let [cljs-db (rust-db->cljs-db rust-db)]
        (log! (str "Restored from legacy format: "
                   (count (d/datoms cljs-db :eavt)) " datom(s)."))
        {:conn    (d/conn-from-db cljs-db)
         :rust-db rust-db}))
    (catch :default e
      (log! (str "Legacy restore failed: " (.-message e)))
      nil)))

;;; DataScript Operations (using CLJS DataScript + WasmDataScript persistence)

(defn- transact! []
  (try
    (let [{:keys [conn]} @state
          text   (.-value (.getElementById js/document "tx-input"))
          report (d/transact! conn (reader/read-string text))]
      (log! (str "Transacted " (count (:tx-data report)) " datom(s)."))
      (render-results! (mapv str (:tx-data report))))
    (catch :default e
      (log! (str "TX ERROR: " (or (.-message e) (str e)))))))

(defn- query! []
  (let [text   (.-value (.getElementById js/document "q-input"))
        q-form (reader/read-string text)
        db     (d/db (:conn @state))]
    (try
      (let [results (d/q q-form db)]
        (log! (str "Query returned " (count results) " result(s)."))
        (render-results! results))
      (catch :default e
        (log! (str "QUERY ERROR: " (.-message e)))))))

(defn- dump-datoms! []
  (let [db     (d/db (:conn @state))
        datoms (vec (d/datoms db :eavt))]
    (log! (str (count datoms) " datom(s) in DB."))
    (render-results! (mapv str datoms))))

(defn- save!
  "Store the current CLJS DB to SQLite via WasmDataScript (Rust-native persistence)."
  []
  (try
    (let [{:keys [conn db-name]} @state
          cljs-db  (d/db conn)
          rust-db  (cljs-db->rust-db! cljs-db)]
      (.storeDb rust-db db-name)
      (swap! state assoc :rust-db rust-db)
      (log! (str "Saved to SQLite via Rust. " (.count rust-db) " datom(s).")))
    (catch :default e
      (log! (str "SAVE ERROR: " (.-message e))))))

(defn- save-legacy!
  "Store the current CLJS DB to SQLite in legacy EDN format (.bp7 compatible)."
  []
  (try
    (let [{:keys [conn db-name]} @state
          cljs-db  (d/db conn)
          rust-db  (cljs-db->rust-db! cljs-db)]
      (.storeToLegacy rust-db db-name)
      (swap! state assoc :rust-db rust-db)
      (log! (str "Saved to SQLite (legacy format). " (.count rust-db) " datom(s).")))
    (catch :default e
      (log! (str "SAVE LEGACY ERROR: " (.-message e))))))

(defn- restore!
  "Restore DB from SQLite. Tries WasmDataScript first, then legacy EDN format."
  []
  (let [{:keys [db-name]} @state]
    (if-let [rust-db (.restoreDb WasmDataScript db-name)]
      (let [cljs-db (rust-db->cljs-db rust-db)
            ds-conn (d/conn-from-db cljs-db)]
        (swap! state assoc :conn ds-conn :rust-db rust-db)
        (set-status! "connected (restored)")
        (log! (str "Restored from SQLite via Rust. " (count (d/datoms cljs-db :eavt)) " datom(s).")))
      ;; Fallback to legacy EDN format (synchronous via Rust)
      (if-let [result (legacy-restore db-name)]
        (do
          (swap! state assoc :conn (:conn result) :rust-db (:rust-db result))
          (set-status! "connected (restored, legacy)"))
        (log! "No stored DB found. Nothing to restore.")))))

;;; Export / Import

(defn- export-db! []
  (let [{:keys [conn sql-conn db-name]} @state]
    ;; Store to SQLite in legacy format for maximum compatibility
    (try
      (let [rust-db (cljs-db->rust-db! (d/db conn))]
        (.storeToLegacy rust-db db-name)
        (swap! state assoc :rust-db rust-db))
      (catch :default e
        (log! (str "STORE ERROR during export: " (.-message e)))))
    (-> (sql/export! sql-conn)
        (p/then (fn [db-bytes]
                  (let [blob (js/Blob. #js [db-bytes] #js {:type "application/octet-stream"})
                        url  (.createObjectURL js/URL blob)
                        a    (.createElement js/document "a")]
                    (set! (.-href a) url)
                    (set! (.-download a) db-name)
                    (.click a)
                    (.revokeObjectURL js/URL url)
                    (log! (str "Exported " (.-length db-bytes) " bytes.")))))
        (p/catch (fn [e] (log! (str "EXPORT ERROR: " (.-message e))))))))

(defn- read-file-bytes
  "Read a File object as Uint8Array. Returns a Promise."
  [file]
  (js/Promise.
   (fn [res _reject]
     (let [rdr (js/FileReader.)]
       (set! (.-onload rdr) (fn [_] (res (js/Uint8Array. (.-result rdr)))))
       (.readAsArrayBuffer rdr file)))))

(defn- import-bytes!
  "Import raw SQLite bytes, then restore DataScript.
   Tries WasmDataScript format first, then legacy EDN format."
  [db-bytes db-name]
  (-> (sql/connect! db-name)
      (p/then (fn [tmp-conn]
                (-> (sql/import! tmp-conn db-bytes)
                    (p/then (fn [_] (sql/close! tmp-conn))))))
      (p/then (fn [_] (sql/connect! db-name)))
      (p/then (fn [fresh-conn]
                (if-let [rust-db (.restoreDb WasmDataScript db-name)]
                  ;; New Rust binary format
                  (let [cljs-db (rust-db->cljs-db rust-db)
                        ds-conn (d/conn-from-db cljs-db)]
                    (swap! state assoc
                           :conn     ds-conn
                           :sql-conn fresh-conn
                           :rust-db  rust-db
                           :db-name  db-name)
                    (set-status! "connected (imported)")
                    (log! (str "Imported " db-name ". "
                               (count (d/datoms cljs-db :eavt)) " datom(s).")))
                  ;; Fallback to legacy EDN format via Rust
                  (if-let [result (legacy-restore db-name)]
                    (do
                      (swap! state assoc
                             :conn     (:conn result)
                             :sql-conn fresh-conn
                             :rust-db  (:rust-db result)
                             :db-name  db-name)
                      (set-status! "connected (imported, legacy)")
                      (log! (str "Imported " db-name " (legacy format).")))
                    (do
                      (swap! state assoc :sql-conn fresh-conn :db-name db-name)
                      (log! (str "Imported " db-name " but no DataScript metadata found.")))))))))

(defn- import-db! []
  (let [input (.createElement js/document "input")]
    (set! (.-type input) "file")
    (set! (.-accept input) ".db,.bp7")
    (.addEventListener
     input "change"
     (fn [_]
       (when-let [file (aget (.-files input) 0)]
         (let [db-name (.-name file)]
           (-> (read-file-bytes file)
               (p/then (fn [db-bytes]
                         (log! (str "Importing " db-name " (" (.-length db-bytes) " bytes)..."))
                         (when-let [old-conn (:sql-conn @state)]
                           (sql/close! old-conn))
                         (import-bytes! db-bytes db-name)))
               (p/catch (fn [e] (log! (str "IMPORT ERROR: " (.-message e))))))))))
    (.click input)))

(defn- gc! []
  (let [{:keys [rust-db db-name]} @state]
    (if rust-db
      (try
        (.collectGarbage rust-db db-name)
        (log! "Garbage collection complete.")
        (catch :default e
          (log! (str "GC ERROR: " (.-message e)))))
      (log! "No Rust DB to GC. Save first."))))

;;; Init

(defn- init-conn!
  "Connect to SQLite and try to restore a DB.
   Tries WasmDataScript binary format first, then legacy EDN, then creates fresh.
   Returns a Promise that resolves to {:conn conn :rust-db rust-db}."
  [schema db-name]
  (if-let [rust-db (.restoreDb WasmDataScript db-name)]
    ;; Rust binary format found
    (let [cljs-db (rust-db->cljs-db rust-db)]
      (log! (str "Restored existing DB (Rust format): " (count (d/datoms cljs-db :eavt)) " datom(s)."))
      (p/resolved {:conn (d/conn-from-db cljs-db) :rust-db rust-db}))
    ;; Try legacy EDN format
    (if-let [result (legacy-restore db-name)]
      (do
        (log! "Restored from legacy EDN format.")
        (p/resolved result))
      (do
        (log! "No existing DB found, creating fresh.")
        (p/resolved {:conn (d/create-conn schema) :rust-db nil})))))

(defn ^:export init []
  (set-status! "initializing SQLite + DataScript...")
  (log! "Starting up (Rust DataScript backend)...")
  (-> (pss/ensure-initialized!)
      (p/then (fn [_] (sql/init!)))
      (p/then (fn [_]
                (let [db-name (:db-name @state)]
                  (-> (sql/connect! db-name)
                      (p/then (fn [sql-conn]
                                (swap! state assoc :sql-conn sql-conn)
                                (-> (init-conn! demo-schema db-name)
                                    (p/then (fn [{:keys [conn rust-db]}]
                                              (swap! state assoc
                                                     :conn     conn
                                                     :rust-db  rust-db)
                                              (set-status! "connected")
                                              (log! "DataScript + SQLite (Rust backend) ready."))))))))))
      (p/catch (fn [e]
                 (let [msg (or (.-message e) (str e))]
                   (set-status! "error")
                   (log! (str "Init failed: " msg))
                   (js/console.error "Init failed:" e)))))

  (.addEventListener (.getElementById js/document "btn-transact")
                     "click" (fn [_] (transact!)))
  (.addEventListener (.getElementById js/document "btn-query")
                     "click" (fn [_] (query!)))
  (.addEventListener (.getElementById js/document "btn-datoms")
                     "click" (fn [_] (dump-datoms!)))
  (.addEventListener (.getElementById js/document "btn-save")
                     "click" (fn [_] (save!)))
  (.addEventListener (.getElementById js/document "btn-restore")
                     "click" (fn [_] (restore!)))
  (.addEventListener (.getElementById js/document "btn-export")
                     "click" (fn [_] (export-db!)))
  (.addEventListener (.getElementById js/document "btn-import")
                     "click" (fn [_] (import-db!)))
  (.addEventListener (.getElementById js/document "btn-gc")
                     "click" (fn [_] (gc!)))
  (.addEventListener (.getElementById js/document "btn-clear")
                     "click" (fn [_]
                               (set! (.-textContent (.getElementById js/document "log")) "")
                               (set! (.-textContent (.getElementById js/document "results")) ""))))
