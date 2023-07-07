(ns fn-mapping
  (:require
   [clojure.java.io        :as io]
   [clojure.set            :as set]
   [clojure.string         :as str]
   [clj-yaml.core          :as yaml]
   [clj-fuzzy.metrics :refer [dice]]
   [datahike.api           :as d]
   [datascript.core        :refer [squuid]]
   [behave-cms.server      :as cms]
   [datom-store.main       :as ds]
   [behave.schema.core     :refer [all-schemas]]
   [behave.schema.queries  :refer [rules q pull-children pull-with-attr]]
   [string-utils.interface :refer [->str ->snake]]
   [map-utils.interface :refer [index-by]]
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

(defn my-empty? [x]
  (cond
    (keyword? x)
    false
    
    (string? x)
    (empty? x)))

(defn ->key [& s]
  (str/join ":" (concat ["behaveplus"] (map (comp ->snake str) (remove my-empty? s)))))

;;; Helpers

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
        name-snake (->snake (get parent name-key))]
    (str/replace (get parent h-or-t-key name-snake) #":help$" "")))

(defn- merge-parent-fields [child entity parent-field parent-id parent]
  (let [gen-attr           #(keyword (str (->str entity) "/" %))
        name-attr          (gen-attr "name")
        translation-attr   (gen-attr "translation-key")
        help-attr          (gen-attr "help-key")
        parent-translation (parent-translation-key parent)
        translation-key    (str parent-translation ":" (->snake (get child name-attr)))
        help-key           (str translation-key ":help")]

    (merge child
           {parent-field     parent-id}
           (when (db-translation-attrs translation-attr) {translation-attr translation-key})
           (when (db-help-attrs help-attr) {help-attr help-key}))))

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

(defn datoms-to-maps [conn]
  (->> (d/datoms conn :eavt)
       (split-datoms)
       (filter #(not (ignore-attrs (second %))))
       (reduce (fn [acc [e a v tx op]]
                 (if op
                   (update-in acc [e a] add-item v)
                   (update-in acc [e a] remove-item v)))
               {})
       (map (fn [[k v]] [k (assoc v :db/id k)]))
       (into (sorted-map))))

(defn pull-and-index-attr [conn attr]
  (index-by attr (pull-with-attr conn attr)))

(defn similar-keys? [m1 m2 & [threshold]]
  (let [k1        (set (keys m1))
        k2        (set (keys m2))
        threshold (or threshold (/ (max (count k1) (count k2)) 2))]
    (> (count (set/intersection k1 k2)) threshold)))

(defn zip [ks vs]
  (apply assoc {} 
         (interleave ks vs)))

(def t-keys {1 :module/translation-key
             3 :submodule/translation-key
             4 :group/translation-key
             5 :group/translation-key
             6 :group/translation-key
             7 :group/translation-key
             8 :group-variable/translation-key})

(defn get-id [& v]
  (let [t-key-attr (get t-keys (count v))
        key        (apply ->key v)]
    (d/q '[:find ?e .
           :in $ ?t-attr ?k
           :where [?e ?t-attr ?k]]
         @@ds/conn t-key-attr key)))

(def exists? (comp some? get-id))

(def cols [:module/name
           :submodule/io
           :submodule/name
           :group/name
           :subgroup-1/name
           :subgroup-2/name
           :subgroup-3/name
           :variable/name])

(def attr-keys {1 [[:module/name] :module :application/_modules]
                3 [[:submodule/io :submodule/name] :submodule :module/_submodules]
                4 [[:group/name] :group :submodule/_groups]
                5 [[:group/name] :group :group/_children]
                6 [[:group/name] :group :group/_children]
                7 [[:group/name] :group :group/_children]})

(defn update-submodule-key [m]
  (let [{io    :submodule/io
         t-key :submodule/translation-key} m
        t-vec                              (str/split t-key #":")
        new-key                            (str/join ":" (concat (take 2 t-vec) [(->str io)] (drop 2 t-vec)))]
    (assoc m
           :submodule/translation-key new-key
           :submodule/help-key (str new-key ":help"))))

(defn exists-or-create [& v]
  (when-not (apply exists? v)
    (let [num-cols                        (count v)
          submodule?                      (= 3 num-cols)
          parent-id                       (if submodule? (get-id (first v)) (apply get-id (butlast v)))
          parent                          (d/pull @@ds/conn '[*] parent-id)
          [attrs entity-type parent-attr] (get attr-keys num-cols)
          child                           (if submodule? (zip attrs (rest v)) (assoc {} (first attrs) (last v)))]
      (cond-> child
        :always 
        (merge-parent-fields
         entity-type
         parent-attr
         parent-id
         parent)

        :always
        (assoc :bp/uuid (str (squuid)))

        submodule?
        (update-submodule-key)))))

(defn search [term corpus]
  (->> corpus
       (map #(assoc {:value %} :match (dice term %)))
       (filter #(< 0.6 (:match %)))
       (sort-by :match)
       (last)))

(comment

  (cms/init-datahike!)

  (def datom-maps (datoms-to-maps @@ds/conn))

  (first datom-maps)

  (similar-keys? {:a "hello" :b "derp" :c "foo"} {:a "hello" :b "no way" :g "maybe?"})

  (spit "db-06-28.yml"
        (yaml/generate-string datoms-to-maps :dumper-options {:flow-style :block}))

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

  (def contain-output-vars
    (q '[:find ?m-name ?io-str ?s-name ?g-name ?v-name ?c-name ?f-name
         :in $ % ?m-name
         :where
         [?m :module/name ?m-name]

         (submodule ?m ?s)
         [?s :submodule/name ?s-name]
         (io ?s ?io)
         [(str ?io) ?io-str]

         (group ?s ?g)
         [?g :group/name ?g-name]

         (variable ?g ?gv ?v)
         [?v :variable/name ?v-name]

         [?gv :group-variable/cpp-class ?c-uuid]
         (uuid ?c ?c-uuid)
         [?c :cpp.class/name ?c-name]

         [?gv :group-variable/cpp-function ?f-uuid]
         (uuid ?f ?f-uuid)
         [?f :cpp.function/name ?f-name]

         (not [?gv :group-variable/cpp-parameter ?p-uuid])]
       @@ds/conn "Contain"))

  (def header [["module/name"
                "submodule/io"
                "submodule/name"
                "group/name"
                "variable/name"
                "cpp/class"
                "cpp/function"
                "cpp/parameter"]])
  
  (spit "contain.tsv" (str/join "\n" (map #(str/join "\t" %) (concat header contain-vars))))
  (spit "contain.tsv" (str/join "\n" (map #(str/join "\t" %) contain-output-vars)) :append true)

  ;; Performed some munging offline

  (def all-vars-w-fns (->> (tsv-parser "var_fns_table.tsv")
                           (map (fn [m] (update m :submodule/io keyword)))))

  ;;; Clear previous CPP classes/fns/params

  (def cpp-attrs [:cpp.parameter/name :cpp.function/name :cpp.class/name])

  (def cpp-ids (flatten (map #(d/q '[:find [?e ...]
                                     :in $ ?attr
                                     :where [?e ?attr]]
                                   @@ds/conn %) cpp-attrs)))

  #_(d/transact @ds/conn (mapv (fn [id] [:db/retractEntity id]) cpp-ids))

  ;;; CPP Mapping

  ;; 1. Get all Namespaces/Classes/Fns/Parameters

  (def namespaces (pull-and-index-attr @@ds/conn :cpp.namespace/name))

  (def global-ns-uuid (:bp/uuid (first (vals namespaces))))

  (def classes (pull-with-attr @@ds/conn
                               :cpp.class/name
                               '[* {:cpp.class/function [* {:cpp.function/parameter [*]}]}]))

  (defn lookup [class-name & [fn-name param-name]]
    (if-let [class (first (filter #(= class-name (:cpp.class/name %)) classes))]
      (cond-> class
        (and (empty? fn-name) (empty? param-name))
        identity

        (some? fn-name)
        (-> (get :cpp.class/function)
            (->> (filter #(= fn-name (:cpp.function/name %))))
            (first))

        (some? param-name)
        (-> (get :cpp.function/parameter)
            (->> (filter #(= param-name (:cpp.parameter/name %))))
            (first)))))

  ;; 2. Filter for variables with a class
  (def vars-wo-fns (filter #(-> % :cpp/class empty?) all-vars-w-fns))
  (count vars-wo-fns)

  (def vars-to-fn-map (remove #(-> % :cpp/class empty?) all-vars-w-fns))

  (defn ->fn-map [m]
    (let [{c :cpp/class f :cpp/function p :cpp/parameter} m
          cpp-class (get-in classes c)]
      (cond-> m

        :always
        (merge 
         {:group-variable/cpp-namespace global-ns-uuid
          :group-variable/cpp-class     (:bp/uuid (lookup c))
          :group-variable/cpp-function  (:bp/uuid (lookup c f))})

        (not (empty? p))
        (assoc :group-variable/cpp-parameter (:bp/uuid (lookup c f p))))))

  (def mapped-vars (map ->fn-map vars-to-fn-map))

  (map (juxt :cpp/class :cpp/function) (filter #(and (not (empty? (:cpp/function %))) (nil? (:group-variable/cpp-function %))) mapped-vars))

  (map (juxt :cpp/class :cpp/function :cpp/parameter) (filter #(and (not (empty? (:cpp/parameter %))) (nil? (:group-variable/cpp-parameter %))) mapped-vars))


  ;;; Major Steps
  ;; 0. Clear out previous data

  ;; -- Remove all group variables
  (def all-group-vars (d/q '[:find  [?e ...]
                         :where [?e :group-variable/translation-key]]
                       @@ds/conn))
  (d/transact @ds/conn (mapv (fn [id] [:db/retractEntity id]) all-group-vars))

  ;; -- Remove all groups
  (def all-groups (d/q '[:find  [?e ...]
                         :where [?e :group/name]]
                       @@ds/conn))
  (d/transact @ds/conn (mapv (fn [id] [:db/retractEntity id]) all-groups))

  ;; Remove all submodules
  (def all-submodules (d/q '[:find  [?e ...]
                             :where [?e :submodule/name]]
                           @@ds/conn))
  (d/transact @ds/conn (mapv (fn [id] [:db/retractEntity id]) all-submodules))

  ;; 1. Ensure the Submodules are in place
  (defn create-submodules [vars]
    (let [num-cols  3
          curr-cols (take num-cols cols)
          curr-data (set (map (fn [row] (mapv #(get row %) curr-cols)) vars))
          to-create (remove #(apply exists? %) (vec curr-data))]
      (mapv #(apply exists-or-create %) to-create)))

  (d/transact @ds/conn (create-submodules mapped-vars))

  ;; 2. Ensure the First Groups under those submodules
  (defn create-groups [vars]
    (let [num-cols  4
          curr-cols (take num-cols cols)
          curr-data (set (map (fn [row] (mapv #(get row %) curr-cols)) vars))
          to-create (remove #(apply exists? %) (vec curr-data))]
      (mapv #(apply exists-or-create %) to-create)))

  (d/transact @ds/conn (create-groups mapped-vars))

  ;; 3. Ensure the 1st Subgroups
  (defn create-subgroups-1 [vars]
    (let [num-cols  5
          curr-cols (take num-cols cols)
          curr-data (set (map (fn [row] (mapv #(get row %) curr-cols)) vars))
          to-create (remove #(apply exists? %) (vec curr-data))]
      (mapv #(apply exists-or-create %) to-create)))

  (d/transact @ds/conn (create-subgroups-1 mapped-vars))

  ;; 4. Ensure the 2nd Subgroups
  (defn create-subgroups-2 [vars]
    (let [num-cols  6
          curr-cols (take num-cols cols)
          curr-data (set (map (fn [row] (mapv #(get row %) curr-cols)) vars))
          to-create (remove #(apply exists? %) (vec curr-data))]
      (mapv #(apply exists-or-create %) to-create)))

  (d/transact @ds/conn (create-subgroups-2 mapped-vars))

  ;; 5. Ensure the 3rd Subgroups exist
  (defn create-subgroups-3 [vars]
    (let [num-cols  7
          curr-cols (take num-cols cols)
          curr-data (set (map (fn [row] (mapv #(get row %) curr-cols)) vars))
          to-create (remove #(apply exists? %) (vec curr-data))]
      (mapv #(apply exists-or-create %) to-create)))

  (d/transact @ds/conn (create-subgroups-3 mapped-vars))

  ;; 6. Resolve Variables to Groups
  (def variables
    (into (sorted-map) (->> (d/datoms @@ds/conn :avet :variable/name)
                            (map (fn [d] [(-> d (nth 2) (str/lower-case)) (nth d 0)])))))

  (def var-names (set (keys variables)))

  (defn find-var [term]
    (let [result (search term var-names)]
      (when result
        (get variables (:value result)))))

  (defn create-group-variable [row]
    (let [{t-key  :key
           v-name :variable/name
           path   :path} row
          search-term    (str/lower-case v-name)]
      (if-let [variable-id (find-var v-name)]
        (let [variable (d/pull @@ds/conn '[*] variable-id)
              group-id (apply get-id (butlast path))
              group    (d/pull @@ds/conn '[*] group-id)]

          (merge
             (select-keys row [:group-variable/cpp-namespace
                               :group-variable/cpp-class
                               :group-variable/cpp-function
                               :group-variable/cpp-parameter])
             {:bp/uuid                        (str (squuid))
              :group/_group-variables         group-id
              :variable/_group-variables      variable-id
              :group-variable/translation-key t-key
              :group-variable/help-key        (str t-key ":help")}))
        [:ERROR search-term])))

  (defn create-group-vars [vars-w-fns]
    (let [num-cols    8
          curr-cols   (take num-cols cols)
          vars-w-keys (map (fn [row]
                             (let [path (map #(get row %) curr-cols)] 
                               (assoc row
                                      :path path
                                      :key (apply ->key path))))
                           vars-w-fns)]
      (map create-group-variable vars-w-keys)))

  (remove :group-variable/cpp-function (create-group-vars mapped-vars))
  (ds/transact @ds/conn (create-group-vars mapped-vars))

  ;; (def config {:store {:backend :file :path "~/.behave_cms/db-new-schema"}})
  ;; (d/create-database (update-in config [:store :path] #(-> % fs/expand-home (.getPath))))
  ;; (def conn (d/connect (update-in config [:store :path] #(-> % fs/expand-home (.getPath)))))
  ;;

  (def submodules (filter (fn [[k v]])))

  (def all-help-w-content
    (q '[:find ?k ?has-content ?written
         :where
         [?h :help-page/key ?k]
         [?h :help-page/content ?c]
         [(count ?c) ?written]
         [(< 10 ?written) ?has-content]
         [(= true ?has-content)]]
       @@ds/conn))
   
  (def all-translation-keys
    (flatten
     (map
      #(q '[:find [?k ...]
            :in $ % ?t-attr
            :where
            [?e ?t-attr ?k]] @@ds/conn %)
      (set (vals t-keys)))))

  (first all-translation-keys)
  )
