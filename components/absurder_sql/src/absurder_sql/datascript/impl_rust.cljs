(ns absurder-sql.datascript.impl-rust
  "Rust DataScript bridge — standalone namespace providing `q`, `pull`, and
   `pull-many` with the same signatures as `datascript.core`, routing through
   the Rust/WASM engine when available and falling back to CLJS DataScript."
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.db :as db]
            [absurder-sql.datascript.impl.entity :as de]
            [absurder-sql.datascript.parser :as dp]
            [clojure.string :as str]))

;;; State

(defonce ^:private state (atom {:rust-db nil}))

(defn set-rust-db!
  "Set the WasmDataScript instance used by `q`, `pull`, and `pull-many`."
  [rust-db]
  (swap! state assoc :rust-db rust-db))

(defn rust-db
  "Return the current WasmDataScript instance, or nil."
  []
  (:rust-db @state))

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

;;; Sync

(defn sync-to-rust!
  "Sync a CLJS DataScript DB to a WasmDataScript instance.
   Extracts all datoms and rebuilds the Rust DB from scratch."
  [cljs-db]
  (let [schema    (:schema cljs-db)
        js-schema (schema->js schema)
        datoms    (d/datoms cljs-db :eavt)
        rust-db   (.emptyDb WasmDataScript js-schema)
        arr       (js/Array.)]
    (doseq [^db/Datom d datoms]
      (.push arr #js {:e  (.-e d)
                      :a  (keyword->attr-str (.-a d))
                      :v  (.-v d)
                      :tx (.-tx d)}))
    (.withDatoms rust-db arr)))

(defn sync-from-rust
  "Create a CLJS DataScript DB from a WasmDataScript instance.
   Extracts all datoms from the Rust DB and creates a CLJS DB via init-db."
  [rust-db]
  (let [js-schema  (.schema rust-db)
        schema     (js->schema js-schema)
        datoms-arr (.datomsIndex rust-db "eavt"
                                 js/undefined js/undefined js/undefined js/undefined
                                 js/undefined js/undefined js/undefined js/undefined)
        datoms     (into []
                         (map (fn [d]
                                (db/datom (.-e d)
                                          (js-datom-attr->keyword (.-a d))
                                          (.-v d)
                                          (.-tx d))))
                         (array-seq datoms-arr))]
    (d/init-db datoms schema)))

;;; Query helpers

(defn- strip-edn-comments
  "Remove ;-style line comments from EDN strings.
   The edn 0.3 Rust crate panics on comments."
  [s]
  (str/replace s #";[^\n]*" ""))

(defn- unsupported-find?
  "Return true if the parsed query uses features the Rust engine can't handle.
   Aggregates are now supported in Rust (Phase 4.1)."
  [_parsed-q]
  false)

(defn- parse-in-bindings
  "Parse :in clause symbols into a vector of role tags.
   $ -> :db, % -> :rules, anything else -> :param with its symbol name."
  [in-clause]
  (mapv (fn [sym]
          (condp = (name sym)
            "$" :db
            "%" :rules
            {:param (name sym)}))
        in-clause))

(defn- js-pull-result->clj
  "Convert a JS pull result (nested objects with ':keyword' string keys)
   to a CLJS map with keyword keys, matching d/pull return format."
  [js-obj]
  (cond
    (nil? js-obj)    nil
    (number? js-obj) js-obj
    (string? js-obj) js-obj
    (boolean? js-obj) js-obj

    (array? js-obj)
    (mapv js-pull-result->clj (array-seq js-obj))

    (object? js-obj)
    (let [entries (js/Object.entries js-obj)]
      (persistent!
       (reduce (fn [m entry]
                 (let [k (aget entry 0)
                       v (aget entry 1)]
                   (assoc! m (keyword (subs k 1)) (js-pull-result->clj v))))
               (transient {})
               (array-seq entries))))

    :else js-obj))

(defn- js-element->clj
  "Convert a single JS query result element to CLJS.
   Pull results (JS objects) get keyword keys; plain values pass through."
  [v]
  (cond
    (nil? v)     nil
    (object? v)  (js-pull-result->clj v)
    (array? v)   (mapv js-element->clj (array-seq v))
    :else        v))

(defn- rust-result->clj
  "Convert JS result from queryEdn to match d/q return format.
   Rel -> set of vectors, Coll/Tuple -> vector, Scalar -> bare value.
   Handles pull-in-find results (nested JS objects) correctly."
  [js-result parsed-q]
  (let [find (:qfind parsed-q)]
    (cond
      (instance? dp/FindScalar find)
      (js-element->clj js-result)

      (instance? dp/FindTuple find)
      (when js-result
        (mapv js-element->clj (array-seq js-result)))

      (instance? dp/FindColl find)
      (when js-result
        (mapv js-element->clj (array-seq js-result)))

      ;; FindRel (default)
      :else
      (into #{}
            (map (fn [row]
                   (mapv js-element->clj (array-seq row))))
            (array-seq js-result)))))

(defn- eid->js
  "Convert a CLJS entity ID to JS for the Rust pull API.
   Handles numeric IDs and lookup refs like [:bp/uuid \"abc\"]."
  [eid]
  (cond
    (number? eid) eid
    ;; Lookup ref: [:attr value]
    (and (vector? eid) (= 2 (count eid)))
    (let [[attr v] eid]
      #js [(keyword->attr-str attr) (clj->js v)])
    :else (clj->js eid)))

;;; Entity API (Phase 4.7)

(defn- rschema-from-schema
  "Build a CLJS rschema map from a CLJS schema map.
   Mirrors db.cljc's rschema structure: {:db.type/ref #{attrs}, :db.cardinality/many #{attrs}, ...}"
  [schema]
  (reduce-kv
    (fn [rs attr props]
      (cond-> rs
        (:db/index props)
        (update :db/index (fnil conj #{}) attr)

        (= :db.unique/identity (:db/unique props))
        (-> (update :db.unique/identity (fnil conj #{}) attr)
            (update :db/unique (fnil conj #{}) attr)
            (update :db/index (fnil conj #{}) attr))

        (= :db.unique/value (:db/unique props))
        (-> (update :db.unique/value (fnil conj #{}) attr)
            (update :db/unique (fnil conj #{}) attr)
            (update :db/index (fnil conj #{}) attr))

        (= :db.cardinality/many (:db/cardinality props))
        (update :db.cardinality/many (fnil conj #{}) attr)

        (= :db.type/ref (:db/valueType props))
        (-> (update :db.type/ref (fnil conj #{}) attr)
            (update :db/index (fnil conj #{}) attr))

        (:db/isComponent props)
        (update :db/isComponent (fnil conj #{}) attr)))
    {} schema))

(defn- js-datoms->clj-datoms
  "Convert a JS array of datom objects to a seq of CLJS Datom records."
  [js-arr]
  (when js-arr
    (map (fn [d]
           (db/datom (.-e d)
                     (js-datom-attr->keyword (.-a d))
                     (.-v d)
                     (.-tx d)))
         (array-seq js-arr))))

(defn- rust-search
  "Search the Rust DB by pattern [e a v tx], returning CLJS Datom records.
   Nils in pattern are wildcards."
  [rdb pattern]
  (let [[e a v tx] pattern
        e-js  (if (some? e) e js/undefined)
        a-js  (if (some? a) (keyword->attr-str a) js/undefined)
        v-js  (if (some? v) (clj->js v) js/undefined)
        tx-js (if (some? tx) tx js/undefined)]
    (js-datoms->clj-datoms (.search rdb e-js a-js v-js tx-js))))

(defn- rust-datoms
  "Get datoms from a named Rust index with bounds."
  [rdb index c0 c1 c2 c3]
  (let [from-e  (if (some? c0) c0 js/undefined)
        from-a  (if (some? c1) (keyword->attr-str c1) js/undefined)
        from-v  js/undefined
        from-tx js/undefined
        to-e    js/undefined
        to-a    js/undefined
        to-v    js/undefined
        to-tx   js/undefined]
    ;; For seek-datoms, we use the index with from bounds only
    (js-datoms->clj-datoms
      (.datomsIndex rdb (name index)
                    from-e from-a from-v from-tx
                    to-e to-a to-v to-tx))))

(deftype RustDBProxy [rdb schema rschema]
  db/IDB
  (-schema [_] schema)
  (-attrs-by [_ property] (get rschema property))

  db/ISearch
  (-search [_ pattern]
    (rust-search rdb pattern))

  db/IIndexAccess
  (-datoms [_ index c0 c1 _c2 _c3]
    (rust-datoms rdb index c0 c1 nil nil))
  (-seek-datoms [_ index c0 c1 _c2 _c3]
    (rust-datoms rdb index c0 c1 nil nil))
  (-rseek-datoms [_ index c0 c1 _c2 _c3]
    (reverse (rust-datoms rdb index c0 c1 nil nil)))
  (-index-range [_ _attr _start _end]
    nil))

(defn- make-rust-db-proxy
  "Create a CLJS-compatible DB proxy wrapping a Rust WasmDataScript instance."
  [rdb schema]
  (let [rschema (rschema-from-schema schema)]
    (->RustDBProxy rdb schema rschema)))

(defn entity
  "Create a DataScript entity backed by the Rust DB.
   Same interface as d/entity. Returns nil if entity doesn't exist."
  ([eid]
   (when-let [rdb (:rust-db @state)]
     (entity rdb eid)))
  ([db-or-rdb eid]
   (if (instance? RustDBProxy db-or-rdb)
     ;; Already a proxy — use directly
     (de/entity db-or-rdb eid)
     ;; Assume it's a WasmDataScript — wrap it
     (when-let [rdb (if (instance? WasmDataScript db-or-rdb)
                      db-or-rdb
                      (:rust-db @state))]
       (let [js-schema (.schema rdb)
             schema    (js->schema js-schema)
             proxy     (make-rust-db-proxy rdb schema)]
         (de/entity proxy eid))))))

(defn touch
  "Eagerly load all attributes of a Rust-backed entity.
   Same interface as d/touch."
  [e]
  (de/touch e))

;;; Transact

(defn- js-tempids->clj
  "Convert a JS tempids object {'-1': 5, ...} to a CLJS map."
  [js-obj]
  (when (and js-obj (not (undefined? js-obj)))
    (let [entries (js/Object.entries js-obj)]
      (into {}
            (map (fn [entry]
                   (let [k (aget entry 0)
                         v (aget entry 1)]
                     ;; Try to parse numeric tempids back to numbers
                     [(let [n (js/parseInt k 10)]
                        (if (js/isNaN n) k n))
                      (long v)])))
            (array-seq entries)))))

(defn- js-tx-datoms->clj
  "Convert a JS array of datom objects from Rust tx-report to CLJS Datom vectors."
  [js-arr]
  (when js-arr
    (into []
          (map (fn [d]
                 (let [tx (.-tx d)]
                   (db/datom (.-e d)
                             (js-datom-attr->keyword (.-a d))
                             (.-v d)
                             (if (neg? tx) (- tx) tx)
                             (pos? tx)))))
          (array-seq js-arr))))

(defn- has-unsupported-tx-forms?
  "Return true if tx-data contains forms that require CLJS (e.g. :db.fn/call, :db.fn/cas)."
  [tx-data]
  (some (fn [entity]
          (when (sequential? entity)
            (let [op (first entity)]
              (or (= op :db.fn/call)
                  (= op :db.fn/cas)
                  (= op :db/cas)
                  ;; Custom tx functions (non-builtin keywords)
                  (and (keyword? op)
                       (not (#{:db/add :db/retract
                               :db.fn/retractAttribute :db.fn/retractEntity
                               :db/retractEntity} op)))))))
        tx-data))

(defn transact-rust!
  "Transact tx-data directly through the Rust engine.
   Returns a map with :tx-data and :tempids, or nil if no rust-db or unsupported forms.
   The rust-db state atom is mutated in place by the Rust engine."
  [tx-data]
  (when-let [rdb (:rust-db @state)]
    (when-not (has-unsupported-tx-forms? tx-data)
      (let [edn-str   (strip-edn-comments (pr-str (vec tx-data)))
            js-report (.transact rdb edn-str)]
        (if (aget js-report "error")
          (throw (js/Error. (str "Rust transact error: " (aget js-report "error"))))
          {:tx-data    (js-tx-datoms->clj (aget js-report "txData"))
           :tempids    (js-tempids->clj (aget js-report "tempids"))
           :current-tx (long (aget js-report "currentTx"))})))))

;;; Public API

(defn q
  "Query via the Rust engine when a rust-db is available.
   Same interface as d/q. Falls back to d/q for unsupported features
   or when no rust-db exists."
  [query-form & inputs]
  (let [rdb      (:rust-db @state)
        parsed-q (dp/parse-query query-form)]
    (if (or (nil? rdb) (unsupported-find? parsed-q))
      (apply d/q query-form inputs)
      (let [q-map      (cond-> query-form
                         (sequential? query-form) dp/query->map)
            in-clause  (or (:in q-map) '[$])
            bindings   (parse-in-bindings in-clause)
            rules-str  (atom nil)
            params     (atom [])
            _          (dorun
                        (map-indexed
                         (fn [idx binding]
                           (let [input (nth inputs idx nil)]
                             (cond
                               (= :db binding)     nil
                               (= :rules binding)  (reset! rules-str
                                                           (strip-edn-comments (pr-str input)))
                               (map? binding)       (swap! params conj
                                                          #js [(str "?" (:param binding))
                                                               (clj->js input)]))))
                         bindings))
            query-edn  (strip-edn-comments (pr-str query-form))
            rules-edn  (or @rules-str "")
            inputs-js  (apply array @params)
            js-result  (.queryEdn rdb query-edn rules-edn inputs-js)]
        (rust-result->clj js-result parsed-q)))))

(defn pull
  "Pull an entity via the Rust engine. Same interface as d/pull.
   Falls back to d/pull if no rust-db is available."
  [db pattern eid]
  (if-let [rdb (:rust-db @state)]
    (let [pattern-edn (strip-edn-comments (pr-str pattern))
          js-result   (.pull rdb pattern-edn (eid->js eid))]
      (js-pull-result->clj js-result))
    (d/pull db pattern eid)))

(defn pull-many
  "Pull multiple entities via the Rust engine. Same interface as d/pull-many.
   Falls back to d/pull-many if no rust-db is available."
  [db pattern eids]
  (if-let [rdb (:rust-db @state)]
    (let [pattern-edn (strip-edn-comments (pr-str pattern))
          eids-js     (apply array (map eid->js eids))
          js-result   (.pullMany rdb pattern-edn eids-js)]
      (mapv js-pull-result->clj (array-seq js-result)))
    (d/pull-many db pattern eids)))
