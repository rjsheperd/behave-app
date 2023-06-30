(ns fn-mapping
  (:require
   [clojure.java.io        :as io]
   [clojure.set            :as set]
   [clojure.string         :as str]
   [clj-yaml.core          :as yaml]
   [datahike.api           :as d]
   [behave-cms.server      :as cms]
   [datom-store.main       :as ds]
   [behave.schema.core     :refer [all-schemas]]
   [behave.schema.queries  :refer [rules q]]
   [string-utils.interface :refer [->str ->kebab]]
   [datom-utils.interface  :refer [split-datoms
                                   safe-attr?
                                   safe-deref]]))

;;; Constants
(def ^:private db-attrs             (map :db/ident all-schemas))
(def ^:private db-translation-attrs (->> db-attrs
                                         (filter #(-> %
                                                      (->str)
                                                      (str/ends-with? "translation-key")))
                                         (set)))
(def ^:private db-help-attrs        (->> db-attrs
                                         (filter #(-> %
                                                      (->str)
                                                      (str/ends-with? "help-key")))
                                         (set)))

(defn- merge-parent-fields [child entity parent-field parent-id parent]
  (let [gen-attr           #(keyword (str (->str entity) "/" %))
        name-attr          (gen-attr "name")
        translation-attr   (gen-attr "translation-key")
        help-attr          (gen-attr "help-key")
        parent-translation (parent-translation-key parent)
        translation-key    (str parent-translation ":" (->kebab (get child name-attr)))
        help-key           (str translation-key ":help")]

    (merge child
           {parent-field     parent-id}
           (when (db-translation-attrs translation-attr) {translation-attr translation-key})
           (when (db-help-attrs help-attr) {help-attr help-key}))))

(defn- parent-translation-key
  "Gets the translation key from `:<parent>/translation-key`,
  `:<parent>/help-key`, or generates it from the `<parent>/name` attribute."
  [parent]
  (let [attrs (map ->str (keys parent)) h-or-t-key (->> attrs
                        (filter #(or (str/ends-with? % "/translation-key")
                                     (str/ends-with? % "/help-key")))
                        (first)
                        (keyword))
        name-key   (->> attrs
                        (filter #(str/ends-with? % "/name"))
                        (first)
                        (keyword))
        name-kebab (->kebab (get parent name-key))]
    (str/replace (get parent h-or-t-key name-kebab) #":help$" "")))

(comment

  (cms/init-datahike!)

  (def datoms (d/datoms @@ds/conn :eavt))

  (def ignore-attrs 
    #{:db/cardinality
      :db/doc
      :db/ident
      :db/index
      :db/tupleAttrs
      :db/txInstant
      :db/unique
      :db/valueType
      :user/name
      :user/super-admin?
      :user/email
      :user/password
      :user/verified?})

  (def datoms-split
    (->> datoms
         (split-datoms)
         (filter #(not (ignore-attrs (second %))))))

  (filter #(str/starts-with? (->str %) "user") (set (map second datoms-split)))
  (seq 3)
  (seq #(3 2 1)

  (first datoms-split)

  (defn add-item [coll x]
    (cond
      (nil? coll)
      x

      (or (vector? coll) (list? coll) (set? coll))
      (conj coll x)

      :else
      [coll x]))

  (defn remove-item [coll x]
      (cond
        (set? coll)
        (disj coll x)

        (vector? coll)
        (->> coll (remove #(= x %)) (vec))

        (list? coll)
        (remove #(= x %) coll)

        (= x coll)
        nil))

  (def datoms-to-maps (->> datoms-split
                           (reduce (fn [acc [e a v tx op]]
                                     (if op
                                       (update-in acc [e a] add-item v)
                                       (update-in acc [e a] remove-item v)))
                                   {})
                           (map (fn [[k v]] [k (assoc v :db/id k)]))
                           (into (sorted-map))))

  (remove-item 2 2)
  (remove-item [3 2] 2)

  (add-item nil 2)
  (add-item [3] 2)
  (add-item 3 2)

  (update-in {:a {:b [3]}} [:a :b] add-item 2)
  (update-in {:a {:b [3 2]}} [:a :b] remove-item 3)

  (spit "db-06-28.yml"
        (yaml/generate-string datoms-to-maps :dumper-options {:flow-style :block}))

  (defn tsv-parser [filename]
    (with-open [rdr (io/reader filename)]
      (let [row-splitter #(-> %
                              (str/split #"\t")
                              (->> (map str/trim)))
            lines        (line-seq rdr)
            header       (map keyword (row-splitter (first lines)))]
        (doall (map (fn [l]
                      (into {} (map (fn [h l] (vector h l)) header (row-splitter l))))
                    (rest lines))))))

  (def var-fns (tsv-parser "var_fns_table.tsv"))

  (set (map :module/name var-fns))
  (def modules (filter (fn [[k v]] (:module/name v)) datoms-to-maps))

  (parent-translation-key (first (vals modules)))

  (q '[:find ?m ?m-name ?s ?s-name ?io ?g ?g-name ?gv ?gv-name
       :keys module/id module/name submodule/id submodule/name submodule/io group/id group/name group-variable/id variable/name 
       :in $ % ?m-name
       :where
       [?m :module/name ?m-name]
       (submodule ?m ?s)
       [?s :submodule/name ?s-name]
       (io ?s ?io)
       (group ?s ?g)
       [?g :group/name ?g-name]
       (variable ?g ?gv)
       (uuid ?gv ?gv-uuid)
       [?v :variable/group-variables ?gv]
       [?v :variable/name ?gv-name]]
     @@ds/conn "Contain")

  (def submodules (filter (fn [[k v]])))

  )

