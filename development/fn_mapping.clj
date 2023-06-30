(ns fn-mapping
  (:require [clojure.string :as str]
            [clojure.set :as set]
            [clj-yaml.core :as yaml]
            [datahike.api :as d]
            [behave-cms.server :as cms]
            [datom-store.main :as ds]
            [string-utils.interface :refer [->str]]
            [datom-utils.interface :refer [split-datoms
                                           safe-attr?
                                           safe-deref]]))

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

  )

