(ns absurder-sql.datascript.impl-rust
  "Rust DataScript bridge — standalone namespace providing `q`, `pull`, and
   `pull-many` with the same signatures as `datascript.core`, routing through
   the Rust/WASM engine when available and falling back to CLJS DataScript."
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.db :as db]
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
  "Return true if the parsed query uses features the Rust engine can't handle
   (aggregates in :find)."
  [parsed-q]
  (let [find-elements (dp/find-elements (:qfind parsed-q))]
    (some dp/aggregate? find-elements)))

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
