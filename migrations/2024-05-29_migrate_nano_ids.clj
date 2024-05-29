
;; 

(comment 
  (require '[clojure.string :as str])
  (require '[clojure.set :as set])
(require '[datomic.api :as d])
(require '[datomic-store.main :as ds]))
(require '[behave-cms.server :as cms])
(require '[behave.schema.core :refer [all-schemas]])
(require '[behave.schema.utils :refer [nid]])

(require '[nano-id.core :refer [nano-id]])

;; Get DB
(cms/init-db!)

;; Transact new Nano-ID Schemas
(def new-schemas (filter #(let [ident (-> % (:db/ident) (name))]
                            (str/ends-with? ident "nid"))
                         all-schemas))

(ds/transact ds/datomic-conn nano-id-schema)

;; Transact Nano-IDSs for existing with UUIDs
(def db (d/db @ds/datomic-conn))

(def eids-w-uuids
  (d/q '[:find  [?e ...]
         :where [?e :bp/uuid ?uuid]] db))

(def nano-id-tx (map (fn [eid] [:db/add eid :bp/nid (nano-id)]) eids-w-uuids))

(ds/transact ds/datomic-conn nano-id-tx)

;; Verify
(def db-after (d/db @ds/datomic-conn))

(def eids-w-nids
  (d/q '[:find  [?e ...]
         :where [?e :bp/nid ?nid]] db-after))

(= (count eids-w-uuids) (count eids-w-nids))

(count (str (d/squuid)))


;; [TODO] Migrate UUID attributes

(def uuid-attrs
  (->> all-schemas
       (filter #(let [ident (-> % (:db/ident) (name))] (str/ends-with? ident "uuid")))
       (map :db/ident)
       (set)))

(def string-attrs
  (->> all-schemas
       (filter #(-> % (:db/valueType) (= :db.type/string)))
       (map :db/ident)
       (set)))

(defn uuid-str? [s]
  (and (string? s) (= 36 (count s)) (uuid? (parse-uuid s))))

(uuid-str? (str (d/squuid)))
(uuid-str? (str/join "" (repeat 36 "d")))

(defn db-uuid? [attr]
  (let [results   (d/q '[:find [?uuid ...]
                       :in $ ?attr
                       :where [_ ?attr ?uuid]]
                     db-after attr)]
    (some uuid-str? results)
    #_(every? uuid-str? results)))

(db-uuid? :group-variable/cpp-function)

(def all-uuid-attrs (concat uuid-attrs (filter db-uuid? (set/difference string-attrs uuid-attrs)))) 

(def uuids-to-remap 
  [[:conditional/group-variable-uuid :conditional/group-variable-nid]
   [:dimension/cpp-enum-uuid :dimension/cpp-enum-nid]
   [:domain/dimension-uuid :domain/dimension-nid]
   [:domain/english-unit-uuid :domain/english-unit-nid]
   [:domain/metric-unit-uuid :domain/metric-unit-nid]
   [:domain/native-unit-uuid :domain/native-unit-nid]
   [:group-variable/cpp-class  :group-variable/cpp-class-nid]
   [:group-variable/cpp-function :group-variable/cpp-function-nid]
   [:group-variable/cpp-namespace :group-variable/cpp-namespace-nid]
   [:group-variable/cpp-parameter :group-variable/cpp-parameter-nid]
   [:subtool-variable/cpp-class-uuid :subtool-variable/cpp-class-nid]
   [:subtool-variable/cpp-function-uuid :subtool-variable/cpp-function-nid]
   [:subtool-variable/cpp-namespace-uuid :subtool-variable/cpp-namespace-nid]
   [:subtool-variable/cpp-parameter-uuid :subtool-variable/cpp-parameter-nid]
   [:subtool/cpp-class-uuid :subtool/cpp-class-nid]
   [:subtool/cpp-function-uuid :subtool/cpp-function-nid]
   [:subtool/cpp-namespace-uuid :subtool/cpp-namespace-nid]
   [:subtool/cpp-parameter-uuid :subtool/cpp-parameter-nid]
   [:unit/cpp-enum-member-uuid :unit/cpp-enum-member-nid]
   [:variable/dimension-uuid :variable/dimension-nid]
   [:variable/domain-uuid :variable/domain-nid]
   [:variable/english-unit-uuid :variable/english-unit-nid]
   [:variable/metric-unit-uuid :variable/metric-unit-nid]
   [:variable/native-unit-uuid :variable/native-unit-nid]])

(defn map-new-nid-attr
  "Creates mapping from a UUID attribute to a Nano-ID attribute."
  [uuid-attr nid-attr]
  (let [source-eids-w-nids (d/q '[:find ?e ?target-nid
                                  :in $ ?attr
                                  :where
                                  [?e ?attr ?target-uuid]
                                  [?target :bp/uuid ?target-uuid]
                                  [?target :bp/nid ?target-nid]]
                                db-after uuid-attr)]
    (map (fn [[eid target-nid]] [:db/add eid nid-attr target-nid]) source-eids-w-nids)))

(def remapping-uuids-tx (apply concat (map #(apply map-new-nid-attr %) uuids-to-remap)))

(def low-eids
  (d/q '[:find (pull ?e [*])
         :in $ [?attr ...]
         :where
         [?e ?attr _]
         [(<= ?e 10000 )]]
       db-after (map :db/ident all-schemas)))

(d/pull db-after '[*] low-eids)


(count remapping-uuids-tx)


;; Worksheet (Later)

:input-group/group-uuid
:input/group-variable-uuid
:output/group-variable-uuid
:result-header/group-variable-uuid
:repeat-group/group-uuid
:table-filter/group-variable-uuid
:graph-settings/x-axis-group-variable-uuid
:graph-settings/z-axis-group-variable-uuid
:graph-settings/z2-axis-group-variable-uuid
:x-axis-limit/group-variable-uuid
:y-axis-limit/group-variable-uuid
:worksheet/uuid
:worksheet.diagram/group-variable-uuid

;;; TODO Links

(def links (d/q '[:find ?e ?s-nid ?d-nid
                  :where
                  [?e :link/source ?s]
                  [?s :bp/nid ?s-nid]
                  [?e :link/destination ?d]
                  [?d :bp/nid ?d-nid]]
                db-after))

;; Assign a NID to each Link, update Source/Destination to use NID's

(def link-nano-ids
  (map (fn [[eid s-nid d-nid]]
         {:db/id                eid
          :bp/nid               (nid)
          :link/source-nid      s-nid
          :link/destination-nid d-nid
          })
       links))

(ds/transact ds/datomic-conn link-nano-ids)
