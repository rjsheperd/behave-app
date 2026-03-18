(ns behave.vms.store
  (:require [ajax.core                  :refer [ajax-request]]
            [ajax.protocols             :as pr]
            ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.impl-rust :as impl-rust]
            [absurder-sql.datascript.persistent-sorted-set :as pss]
            [clojure.string             :as str]
            [posh.reagent               :refer [pull pull-many q posh!]
             :rename {q posh-query pull posh-pull pull-many posh-pull-many}]
            [promesa.core               :as p]
            [re-frame.core              :as rf]
            [datom-compressor.interface :as c]
            [ds-schema-utils.interface  :refer [->ds-schema]]
            [behave.schema.core         :refer [all-schemas]]
            [behave.translate           :refer [load-translations!]]))

;;; State

(defonce vms-conn (atom nil))

;;; IndexedDB VMS Cache

(def ^:private idb-name "behave-vms-cache")
(def ^:private idb-store "vms")

(defn- idb-open []
  (js/Promise.
   (fn [resolve reject]
     (let [req (.open js/indexedDB idb-name 1)]
       (set! (.-onupgradeneeded req)
             (fn [e]
               (let [db (.. e -target -result)]
                 (when-not (.contains (.-objectStoreNames db) idb-store)
                   (.createObjectStore db idb-store)))))
       (set! (.-onsuccess req) (fn [e] (resolve (.. e -target -result))))
       (set! (.-onerror req) (fn [e] (reject (.. e -target -error))))))))

(defn- idb-get-cache [version]
  (-> (idb-open)
      (p/then (fn [db]
                (js/Promise.
                 (fn [resolve _reject]
                   (let [tx  (.transaction db idb-store "readonly")
                         os  (.objectStore tx idb-store)
                         req (.get os version)]
                     (set! (.-onsuccess req)
                           (fn [e]
                             (.close db)
                             (resolve (.. e -target -result))))
                     (set! (.-onerror req)
                           (fn [_]
                             (.close db)
                             (resolve nil))))))))
      (p/catch (fn [_] nil))))

(defn- idb-set-cache! [version body]
  (-> (idb-open)
      (p/then (fn [db]
                (let [tx (.transaction db idb-store "readwrite")
                      os (.objectStore tx idb-store)]
                  (.clear os)
                  (.put os body version)
                  (.close db))))
      (p/catch (fn [e] (js/console.warn "VMS cache write failed:" e)))))

;;; Helpers

(defn- db-attr?
  "Returns true if keyword starts with :db or :fressian."
  [k]
  (let [s (str k)]
    (or (str/starts-with? s ":db")
        (str/starts-with? s ":fressian"))))

(defn- raw-datoms->map
  "Single-pass reduce: filters out :db/* / :fressian* attrs and nil values,
   and accumulates into an entity map keyed by entity id."
  [raw-datoms]
  (let [entities (persistent!
                  (reduce (fn [acc [e a v]]
                            (if (or (db-attr? a) (nil? v))
                              acc
                              (if-let [entity (get acc e)]
                                (let [cur (get entity a)
                                      val (cond
                                            (coll? cur) (conj cur v)
                                            (some? cur) (vector cur v)
                                            :else       v)]
                                  (assoc! acc e (assoc entity a val)))
                                (assoc! acc e {a v}))))
                          (transient {})
                          raw-datoms))]
    (sort-by :db/id
             (map (fn [[idx m]] (assoc m :db/id idx)) entities))))

(defn- process-and-init! [body version]
  (let [datoms-map (raw-datoms->map (c/unpack body))]
    (rf/dispatch-sync [:vms/initialize (->ds-schema all-schemas) datoms-map])
    (rf/dispatch-sync [:state/set :vms-loaded? true])
    (load-translations!)
    (when version
      (idb-set-cache! version (js/Uint8Array. body)))))

(defn- fetch-vms! [version]
  (ajax-request {:uri             (str "/layout.msgpack?v=" version)
                 :handler         (fn [[ok body]]
                                   (when ok
                                     (process-and-init! body version)))
                 :format          {:content-type "application/text" :write str}
                 :response-format {:description  "ArrayBuffer"
                                   :type         :arraybuffer
                                   :content-type "application/msgpack"
                                   :read         pr/-body}}))

(defn- reloaded-vms-data [[ok _]]
  (when ok
    (rf/dispatch-sync [:state/set :vms-reloaded? true])))

;;; Public Fns

(defn load-vms! [version]
  (-> (p/all [(pss/ensure-initialized!) (idb-get-cache version)])
      (p/then (fn [[_ cached]]
                (if cached
                  (process-and-init! cached nil)
                  (fetch-vms! version))))
      (p/catch (fn [_]
                 (fetch-vms! version)))))

(defn reload-vms! []
  (ajax-request {:uri     "/api/vms-sync"
                 :handler reloaded-vms-data}))

;;; Public Fns

(defn- entity-maps->cljs-datoms
  "Expand entity maps [{:db/id 1 :name \"Alice\"} ...] into a vector of CLJS
   Datom objects. Handles multi-valued attributes (vectors/sets → one datom per
   element). Pure CLJS, no WASM calls."
  [entities]
  (let [tx absurder-sql.datascript.db/tx0]
    (persistent!
      (reduce
        (fn [acc entity]
          (let [eid (:db/id entity)]
            (reduce-kv
              (fn [acc k v]
                (if (= k :db/id)
                  acc
                  (if (and (coll? v) (not (string? v)) (not (map? v)))
                    ;; Multi-valued: one datom per element
                    (reduce (fn [acc elem]
                              (conj! acc (absurder-sql.datascript.db/datom eid k elem tx)))
                            acc v)
                    (conj! acc (absurder-sql.datascript.db/datom eid k v tx)))))
              acc entity)))
        (transient [])
        entities))))

(defn- entity-maps->datom-array
  "Convert a seq of entity maps [{:db/id 1 :name \"Alice\"} ...] to a flat
   JS array of {e, a, v, tx} datom objects suitable for WasmDataScript.withDatoms.
   Handles multi-valued attributes (vectors/sets → one datom per element)."
  [entities schema]
  (let [arr     (js/Array.)
        ref-attrs (into #{}
                    (keep (fn [[k v]]
                            (when (= :db.type/ref (:db/valueType v)) k)))
                    schema)]
    (doseq [entity entities
            :let [eid (:db/id entity)]
            [k v] (dissoc entity :db/id)
            :let [a-str (if (namespace k)
                          (str ":" (namespace k) "/" (name k))
                          (str ":" (name k)))]
            ;; Expand multi-valued attrs (vectors/sets) into individual datoms
            val (if (and (or (vector? v) (set? v)) (not (string? v)))
                  v
                  [v])]
      (.push arr #js {:e  eid
                      :a  a-str
                      :v  (cond
                            (keyword? val) (if (namespace val)
                                            (str ":" (namespace val) "/" (name val))
                                            (str ":" (name val)))
                            (and (integer? val) (contains? ref-attrs k)) val
                            :else val)
                      :tx 536870913}))
    arr))

(defn init! [{:keys [datoms schema]}]
  (if @vms-conn
    @vms-conn
    ;; Fast path: expand entity maps → CLJS Datom objects (pure CLJS, no WASM),
    ;; then init-db builds all 3 indexes via from-sorted-array (3 WASM calls).
    ;; ~100x fewer WASM boundary crossings than d/transact.
    (let [cljs-dats (entity-maps->cljs-datoms datoms)
          cljs-db   (d/init-db cljs-dats schema)
          conn      (d/conn-from-db cljs-db)]
      (reset! vms-conn conn)
      ;; Build Rust DB deferred (non-blocking, VMS is read-only)
      (js/setTimeout
        (fn []
          (let [rdb (impl-rust/sync-to-rust! @conn)]
            (impl-rust/set-rust-db! rdb @conn)))
        0)
      (posh! conn)
      ;; Force posh to evaluate queries against the pre-loaded DB.
      ;; Without this, posh's lazy reactions see an empty result because
      ;; init-db bypassed d/transact (no after-transact event fired).
      (d/transact conn [[:db/add 0 :posh/init true]])
      (d/transact conn [[:db/retract 0 :posh/init true]])
      conn)))

;;; Effects

(rf/reg-fx :vms/init init!)

;;; Events

(rf/reg-event-fx
 :vms/initialize
 (fn [_ [_ schema datoms]]
   {:vms/init {:datoms datoms :schema schema}}))

;;; Operations
(defn q [query & variables]
  (apply posh-query query @vms-conn variables))

(defn pull [pattern id]
  (posh-pull @vms-conn pattern id))

(defn pull-many [pattern ids]
  (posh-pull-many @vms-conn pattern ids))

(defn entity-from-uuid
  "Return a re-frame entity using a UUID (maps to the `:bp/uuid` attribute)"
  [bp-uuid]
  (or (impl-rust/entity [:bp/uuid bp-uuid])
      (d/entity @@vms-conn [:bp/uuid bp-uuid])))

(defn entity-from-nid
  "Return a re-frame entity using a Nano-ID (maps to the `:bp/nid` attribute)"
  [bp-nid]
  (or (impl-rust/entity [:bp/nid bp-nid])
      (d/entity @@vms-conn [:bp/nid bp-nid])))

(defn entity-from-eid
  "Return a re-frame entity using an entity ID (maps to the `:db/id` attribute)"
  [eid]
  (or (impl-rust/entity eid)
      (d/entity @@vms-conn eid)))
