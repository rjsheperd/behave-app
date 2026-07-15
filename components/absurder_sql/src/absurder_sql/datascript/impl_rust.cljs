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

(defonce ^:private state (atom {:rust-db        nil
                                :cljs-db        nil
                                :named-dbs      {}
                                :named-cljs-dbs {}}))

;; Strong reference set prevents premature GC of WasmDataScript objects.
;; wasm-bindgen's FinalizationRegistry can free WASM objects when JS GC
;; doesn't trace references through CLJS persistent data structures.
(defonce ^:private live-rdbs (js/Set.))

(defonce rust-enabled? (atom true))

(defn set-rust-db!
  "Set the default WasmDataScript instance (VMS) used by `q`, `pull`, and `pull-many`.
   Optionally pass the CLJS DB value for smart query routing (Phase 3)."
  ([rust-db]
   (when-let [old (:rust-db @state)] (.delete live-rdbs old))
   (when rust-db (.add live-rdbs rust-db))
   (swap! state assoc :rust-db rust-db))
  ([rust-db cljs-db]
   (when-let [old (:rust-db @state)] (.delete live-rdbs old))
   (when rust-db (.add live-rdbs rust-db))
   (swap! state assoc :rust-db rust-db :cljs-db cljs-db)))

(defn rust-db
  "Return the default WasmDataScript instance, or nil."
  []
  (:rust-db @state))

(defn set-named-db!
  "Register a named WasmDataScript instance (e.g. \"$ws\" for worksheet).
   Optionally pass the CLJS DB value for smart query routing (Phase 3)."
  ([db-name rust-db]
   (when-let [old (get-in @state [:named-dbs db-name])] (.delete live-rdbs old))
   (when rust-db (.add live-rdbs rust-db))
   (swap! state assoc-in [:named-dbs db-name] rust-db))
  ([db-name rust-db cljs-db]
   (when-let [old (get-in @state [:named-dbs db-name])] (.delete live-rdbs old))
   (when rust-db (.add live-rdbs rust-db))
   (swap! state #(-> %
                     (assoc-in [:named-dbs db-name] rust-db)
                     (assoc-in [:named-cljs-dbs db-name] cljs-db)))))

(defn named-db
  "Return the WasmDataScript instance for `db-name`, or nil."
  [db-name]
  (get-in @state [:named-dbs db-name]))

(defn remove-named-db!
  "Remove a named WasmDataScript instance."
  [db-name]
  (when-let [old (get-in @state [:named-dbs db-name])] (.delete live-rdbs old))
  (swap! state #(-> %
                    (update :named-dbs dissoc db-name)
                    (update :named-cljs-dbs dissoc db-name))))

(defn clear-all-state!
  "Reset all WASM state. Call on page reload to clear stale references
   from a previous page load before WASM memory is re-initialized."
  []
  (js/console.log "[impl-rust] clear-all-state! called"
                  (js/Error. "stack trace"))
  (.clear live-rdbs)
  (reset! state {:rust-db        nil
                 :cljs-db        nil
                 :named-dbs      {}
                 :named-cljs-dbs {}}))

;; Console toggle for dev
(when (exists? js/window)
  (set! js/window.toggleRustEngine
        (fn []
          (swap! rust-enabled? not)
          (js/console.log (str "Rust engine " (if @rust-enabled? "ENABLED" "DISABLED"))))))

;;; Schema conversion (CLJS <-> JS for WasmDataScript)

(defn schema->js
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
   :person/name -> \":person/name\", :name -> \":name\".
   Also handles string attributes (passed through with leading colon)."
  [kw]
  (if (keyword? kw)
    (if (namespace kw)
      (str ":" (namespace kw) "/" (name kw))
      (str ":" (name kw)))
    (if (string? kw)
      (if (= (aget kw 0) ":")
        kw
        (str ":" kw))
      (str ":" kw))))

(defn- js-datom-attr->keyword
  "Convert a JS datom attribute string like ':name' or ':ns/name' to a keyword."
  [^string s]
  (keyword (subs s 1)))

(defn- js-datom-value->clj
  "Convert a JS datom value to CLJS.

  Rust stringifies keyword values as ':name' / ':ns/name'; convert those
  back to keywords. Numbers, plain strings, and booleans pass through
  unchanged.

  Note: a genuine string value starting with ':' is indistinguishable from
  a keyword here and will be coerced. This mirrors the inverse heuristic in
  `value_from_js` (wasm.rs) and is acceptable for behave data."
  [v]
  (if (and (string? v) (identical? ":" (.charAt ^string v 0)))
    (keyword (subs v 1))
    v))

;;; Sync

(defn- escape-bulk-str
  "Escape tab and newline characters in string values for bulk serialization.
   Rust side unescapes with the inverse transform."
  [^string s]
  (if (and (not (.includes s "\t")) (not (.includes s "\n")))
    s
    (-> s
        (.replace (js/RegExp. "\\\\" "g") "\\\\")
        (.replace (js/RegExp. "\t" "g") "\\t")
        (.replace (js/RegExp. "\n" "g") "\\n"))))

(defn value-type+str
  "Encode a value as type tag + string for bulk serialization to Rust."
  [v]
  (cond
    (string? v)  (str "s\t" (escape-bulk-str v))
    (number? v)  (str "n\t" v)
    (keyword? v) (str "k\t" (if-let [ns (namespace v)]
                              (str ns "/" (name v))
                              (name v)))
    (boolean? v) (str "b\t" (if v "true" "false"))
    (nil? v)     "s\t"
    :else        (str "s\t" (escape-bulk-str (str v)))))

(defn sync-to-rust!
  "Sync a CLJS DataScript DB to a WasmDataScript instance.
   Uses bulk string serialization — one WASM call for all datoms."
  [cljs-db]
  (let [schema    (:schema cljs-db)
        js-schema (schema->js schema)
        datoms    (d/datoms cljs-db :eavt)
        rust-db   (.emptyDb WasmDataScript js-schema)
        sb        (js/Array.)]
    (doseq [^db/Datom d datoms]
      (let [a     (.-a d)
            a-str (if (keyword? a)
                    (if-let [ns (namespace a)]
                      (str ns "/" (name a))
                      (name a))
                    (str a))]
        (.push sb (str (.-e d) "\t" a-str "\t" (value-type+str (.-v d)) "\t" (.-tx d)))))
    (.transactBulkString rust-db (.join sb "\n"))
    rust-db))

(defn apply-tx-data!
  "Apply resolved tx-report datoms incrementally to an existing Rust DB.
   Much faster than full sync-to-rust! for small transactions on large DBs.
   tx-data is a seq of CLJS Datom objects (from tx-report :tx-data)."
  [rust-db tx-data]
  (let [sb (js/Array.)]
    (doseq [^db/Datom d tx-data]
      (let [a     (.-a d)
            a-str (if (keyword? a)
                    (if-let [ns (namespace a)]
                      (str ns "/" (name a))
                      (name a))
                    (str a))
            ;; Positive tx = add, negative tx = retract (matches Rust convention)
            tx    (if (db/datom-added d) (.-tx d) (- (.-tx d)))]
        (.push sb (str (.-e d) "\t" a-str "\t" (value-type+str (.-v d)) "\t" tx))))
    (.applyTxData rust-db (.join sb "\n"))))

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
                                          (js-datom-value->clj (.-v d))
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
   $ -> :db, % -> :rules, $ws -> {:source \"$ws\"}, anything else -> :param."
  [in-clause]
  (mapv (fn [sym]
          (let [s (if (symbol? sym) (name sym) (str sym))]
            (cond
              (= s "$")              :db
              (= s "%")              :rules
              (str/starts-with? s "$") {:source s}
              :else                    {:param s})))
        in-clause))

(defn- str->keyword
  "Convert a string like \":input\" or \":db.cardinality/many\" to a keyword."
  [^string s]
  (let [s (if (identical? ":" (.charAt s 0)) (.substring s 1) s)
        i (.indexOf s "/")]
    (if (== i -1)
      (keyword s)
      (keyword (.substring s 0 i) (.substring s (inc i))))))

(defn- js-pull-result->clj
  "Convert a JS pull result (nested objects with ':keyword' string keys)
   to a CLJS map with keyword keys, matching d/pull return format.
   String values starting with \":\" are converted back to keywords."
  [js-obj]
  (cond
    (nil? js-obj)    nil
    (number? js-obj) js-obj
    (string? js-obj) (if (identical? ":" (.charAt ^string js-obj 0))
                       (str->keyword js-obj)
                       js-obj)
    (boolean? js-obj) js-obj

    (array? js-obj)
    (mapv js-pull-result->clj (array-seq js-obj))

    (object? js-obj)
    (let [entries (js/Object.entries js-obj)]
      (persistent!
       (reduce (fn [m entry]
                 (let [k (aget entry 0)
                       v (aget entry 1)]
                   (assoc! m (str->keyword k) (js-pull-result->clj v))))
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
        e-js       (if (some? e) e js/undefined)
        a-js       (if (some? a) (keyword->attr-str a) js/undefined)
        v-js       (if (some? v) (clj->js v) js/undefined)
        tx-js      (if (some? tx) tx js/undefined)]
    (js-datoms->clj-datoms (.search rdb e-js a-js v-js tx-js))))

(defn- rust-datoms
  "Get datoms from a named Rust index with bounds.
   c0-c3 are in index-specific order (DataScript convention):
     :eavt → c0=e, c1=a, c2=v, c3=tx
     :aevt → c0=a, c1=e, c2=v, c3=tx
     :avet → c0=a, c1=v, c2=e, c3=tx
   Maps them to the (e, a, v, tx) order that .datomsIndex expects."
  [rdb index c0 c1 c2 c3]
  (let [;; Map index-specific component ordering to (e, a, v, tx)
        [e a v tx] (case index
                     (:eavt "eavt") [c0 c1 c2 c3]
                     (:aevt "aevt") [c1 c0 c2 c3]
                     (:avet "avet") [c2 c0 c1 c3]
                     [c0 c1 c2 c3])
        from-e     (if (some? e) e js/undefined)
        from-a     (if (some? a) (keyword->attr-str a) js/undefined)
        from-v     (if (some? v) (clj->js v) js/undefined)
        from-tx    (if (some? tx) tx js/undefined)
        to-e       js/undefined
        to-a       js/undefined
        to-v       js/undefined
        to-tx      js/undefined]
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
  (-datoms [_ index c0 c1 c2 c3]
    (rust-datoms rdb index c0 c1 c2 c3))
  (-seek-datoms [_ index c0 c1 c2 c3]
    (rust-datoms rdb index c0 c1 c2 c3))
  (-rseek-datoms [_ index c0 c1 c2 c3]
    (reverse (rust-datoms rdb index c0 c1 c2 c3)))
  (-index-range [_ _attr _start _end]
    nil))

(defn- make-rust-db-proxy
  "Create a CLJS-compatible DB proxy wrapping a Rust WasmDataScript instance."
  [rdb schema]
  (let [rschema (rschema-from-schema schema)]
    (->RustDBProxy rdb schema rschema)))

(defn entity
  "Create a DataScript entity backed by the Rust DB.
   Same interface as d/entity. Returns nil if entity doesn't exist or rust-enabled? is false."
  ([eid]
   (when @rust-enabled?
     (when-let [rdb (:rust-db @state)]
       (entity rdb eid))))
  ([db-or-rdb eid]
   (if (instance? RustDBProxy db-or-rdb)
     ;; Already a proxy — use directly
     (de/entity db-or-rdb eid)
     ;; Assume it's a WasmDataScript — wrap it
     (when-let [rdb (if (instance? WasmDataScript db-or-rdb)
                      db-or-rdb
                      (when @rust-enabled? (:rust-db @state)))]
       (let [js-schema (.schema rdb)
             schema    (js->schema js-schema)
             proxy     (make-rust-db-proxy rdb schema)]
         (de/entity proxy eid))))))

(defn entity-ws
  "Create a DataScript entity backed by the worksheet ('$ws') Rust DB.
   Returns nil if rust-enabled? is false or no '$ws' DB exists."
  [eid]
  (when @rust-enabled?
    (when-let [rdb (get-in @state [:named-dbs "$ws"])]
      (entity rdb eid))))

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

(defn- q-impl
  "Core Rust query implementation. Uses `rdb` as the primary DB."
  [rdb query-form inputs]
  (let [parsed-q    (dp/parse-query query-form)
        q-map       (cond-> query-form
                      (sequential? query-form) dp/query->map)
        in-clause   (or (:in q-map) '[$])
        bindings    (parse-in-bindings in-clause)
        rules-str   (atom nil)
        params      (atom [])
        source-name (atom "")
        source-db   (atom nil)
        _           (dorun
                     (map-indexed
                      (fn [idx binding]
                        (let [input (nth inputs idx nil)]
                          (cond
                            (= :db binding)      nil
                            (= :rules binding)   (reset! rules-str
                                                         (strip-edn-comments (pr-str input)))
                            (:source binding)     (let [src-name (:source binding)]
                                                    (reset! source-name src-name)
                                                    (cond
                                                      (instance? WasmDataScript input)
                                                      (reset! source-db input)

                                                     ;; Look up named DB if input is a CLJS DB
                                                      :else
                                                      (when-let [ndb (get-in @state [:named-dbs src-name])]
                                                        (reset! source-db ndb))))
                            (:param binding)      (swap! params conj
                                                         #js [(str "?" (:param binding))
                                                              (clj->js input)]))))
                      bindings))
        query-edn   (strip-edn-comments (pr-str query-form))
        rules-edn   (or @rules-str "")
        inputs-js   (apply array @params)
        ;; queryEdnMulti consumes source_db (wasm-bindgen zeros __wbg_ptr)
        ;; but returns [result, source_db] so we can re-register it.
        js-result   (if (seq @source-name)
                      (let [ret         (.queryEdnMulti rdb query-edn rules-edn inputs-js
                                                        @source-name @source-db)
                            result      (aget ret 0)
                            returned-db (aget ret 1)]
                        ;; Re-register the returned source DB (new JS wrapper, same Rust struct)
                        (when (and returned-db (not (nil? returned-db)))
                          (swap! state assoc-in [:named-dbs @source-name] returned-db)
                          (.add live-rdbs returned-db))
                        result)
                      (.queryEdn rdb query-edn rules-edn inputs-js))]
    (rust-result->clj js-result parsed-q)))

(defn- resolve-rust-db
  "Given the first input (the $ DB), find the correct WasmDataScript instance.
   Checks stored CLJS DB identity to route posh queries to the right Rust DB.
   Falls back to the default Rust DB when no identity match is found (for q)."
  [db-input]
  (let [s @state]
    (or
     ;; Check if DB matches the default VMS CLJS DB
     (when (and (:cljs-db s) db-input (identical? db-input (:cljs-db s)))
       (:rust-db s))
     ;; Check named CLJS DBs (e.g. "$ws" worksheet)
     (some (fn [[db-name cljs-db]]
             (when (and cljs-db (identical? db-input cljs-db))
               (get (:named-dbs s) db-name)))
           (:named-cljs-dbs s))
     ;; Default to VMS Rust DB
     (:rust-db s))))

(defn- resolve-rust-db-strict
  "Like resolve-rust-db, but returns nil when no identity match is found.
   Used by pull/pull-many/entid to avoid querying a stale Rust DB after
   d/transact! creates a new CLJS DB value that no longer matches."
  [db-input]
  (let [s @state]
    (or
     (when (and (:cljs-db s) db-input (identical? db-input (:cljs-db s)))
       (:rust-db s))
     (some (fn [[db-name cljs-db]]
             (when (and cljs-db (identical? db-input cljs-db))
               (get (:named-dbs s) db-name)))
           (:named-cljs-dbs s)))))

(defn- has-db-input?
  "Returns true if the query's :in clause expects a database source ($).
   Queries without a DB (e.g. posh's internal collection re-queries with
   :in [[vars ...]]) should always fall back to CLJS d/q."
  [query-form]
  (let [q-map     (cond-> query-form
                    (sequential? query-form) dp/query->map)
        in-clause (or (:in q-map) '[$])]
    (some #(or (= '$ %) (and (symbol? %) (str/starts-with? (name %) "$")))
          in-clause)))

(defn- has-attr-param?
  "Returns true if any :where clause uses an input parameter in the attribute
   position. Handles both standard [e a v] and posh-normalized [$ e a v] forms.
   The Rust engine cannot bind keyword input params to attribute positions."
  [query-form]
  (let [q-map        (cond-> query-form
                       (sequential? query-form) dp/query->map)
        in-clause    (or (:in q-map) '[$])
        input-params (into #{} (filter #(and (symbol? %)
                                             (not= '$ %)
                                             (not= '% %)
                                             (not (str/starts-with? (name %) "$"))))
                           in-clause)
        where-clause (:where q-map)]
    (some (fn [clause]
            (when (vector? clause)
              (let [;; Posh normalizes clauses to [$ e a v], standard is [e a v]
                    attr-idx (cond
                               ;; [$ e a ...] — posh normalized, attr at index 2
                               (and (>= (count clause) 3)
                                    (= '$ (first clause)))
                               2
                               ;; [e a ...] — standard, attr at index 1
                               (>= (count clause) 2)
                               1
                               :else nil)]
                (when attr-idx
                  (contains? input-params (nth clause attr-idx))))))
          where-clause)))

(defn q
  "Query via the Rust engine when a rust-db is available.
   Same interface as d/q. Falls back to d/q for unsupported features
   or when no rust-db exists. Supports multi-source queries (`:in $ $ws`)
   by routing through `queryEdnMulti` and looking up named DBs.
   Detects the correct Rust DB by comparing the input DB identity with
   tracked CLJS DBs (for posh/re-posh reactive query routing)."
  [query-form & inputs]
  (if (or (not @rust-enabled?)
          (not (has-db-input? query-form))
          (has-attr-param? query-form))
    (apply d/q query-form inputs)
    (let [rdb (resolve-rust-db (first inputs))]
      (if (nil? rdb)
        (apply d/q query-form inputs)
        (q-impl rdb query-form inputs)))))

(defn q-ws
  "Query using the worksheet Rust DB ('$ws') as the primary source.
   Falls back to d/q when rust-enabled? is false or no '$ws' DB exists."
  [query-form & inputs]
  (let [rdb (get-in @state [:named-dbs "$ws"])]
    (if (or (not @rust-enabled?) (nil? rdb))
      (apply d/q query-form inputs)
      (q-impl rdb query-form inputs))))

(defn pull
  "Pull an entity via the Rust engine. Same interface as d/pull.
   Falls back to d/pull if the CLJS DB doesn't match a registered Rust DB
   (e.g. after d/transact! creates a new DB value)."
  [db pattern eid]
  (let [rdb (when @rust-enabled? (resolve-rust-db-strict db))]
    (if (nil? rdb)
      (d/pull db pattern eid)
      (let [pattern-edn (strip-edn-comments (pr-str pattern))
            js-result   (.pull rdb pattern-edn (eid->js eid))]
        (js-pull-result->clj js-result)))))

(defn pull-many
  "Pull multiple entities via the Rust engine. Same interface as d/pull-many.
   Falls back to d/pull-many if the CLJS DB doesn't match a registered Rust DB
   (e.g. after d/transact! creates a new DB value)."
  [db pattern eids]
  (let [rdb (when @rust-enabled? (resolve-rust-db-strict db))]
    (when js/goog.DEBUG
      (js/console.log "[pull-many]" "rust?" (some? rdb)
                      "pattern:" (pr-str pattern)
                      "eids:" (pr-str (take 3 eids))
                      "db-type:" (type db)
                      "db=conn?" (identical? db (:cljs-db @state))))
    (if (nil? rdb)
      (let [result (d/pull-many db pattern eids)]
        (when js/goog.DEBUG
          (js/console.log "[pull-many] CLJS result sample:"
                          (pr-str (take 2 result))))
        result)
      (let [pattern-edn (strip-edn-comments (pr-str pattern))
            eids-js     (apply array (map eid->js eids))
            js-result   (.pullMany rdb pattern-edn eids-js)]
        (mapv js-pull-result->clj (array-seq js-result))))))

(defn entid
  "Resolve an entity ID or lookup ref to a numeric eid via the Rust engine.
   Falls back to d/entid if the CLJS DB doesn't match a registered Rust DB."
  [db eid]
  (let [rdb (when @rust-enabled? (resolve-rust-db-strict db))]
    (if (nil? rdb)
      (d/entid db eid)
      (let [result (.entid rdb (eid->js eid))]
        (when-not (nil? result)
          (long result))))))
