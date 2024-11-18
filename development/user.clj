(ns user)

(comment
  (require '[behave.core :as core]
           '[config.interface :refer [get-config load-config]])

  (core/init-config!)
  (core/init-db! (get-config :database))

  (core/vms-sync!)

  (require '[behave-cms.server :as cms])
  (cms/init-db!)

  (require '[clj-http.client :as client])
  (require '[me.raynes.fs :as fs])
  (require '[clojure.java.io :as io])
  (require '[clojure.edn :as edn])
  (require '[clojure.string :as str])
  (require '[behave.schema.core :refer [all-schemas]])
  (require '[datomic.api :as d])
  (require '[datomic-store.main :as ds])
  (require '[datom-utils.interface :refer [split-datoms]])

  (def db (d/db @ds/datomic-conn))

  (def var-ids (d/q '[:find [?e ...]
                      :where
                      [?e :variable/kind :continuous]
                      [?e :variable/name ?v]] db))

  (def entities (map (partial d/entity db) var-ids))

  (def vars (map (juxt :db/id :variable/name :variable/bp6-code) entities))

  (spit "variables.edn" (pr-str vars))
  (def vars (edn/read-string (slurp "variables.edn")))

  (require '[clj-fuzzy.metrics :refer [jaro]])

  (defn n-matches [n match variables]
    (take n (sort-by (fn [[id v-name _code]]
                       (- 1.0 (jaro v-name match))) variables)))

  (n-matches 5 "1-h Fuel Load" vars)
  (n-matches 5 "10-h Fuel Load" vars)
  (n-matches 5 "Aspen Fuel Curing Level" vars)
  (n-matches 5 "Age of Rough" vars)


  (require '[csv-parser.interface :refer [parse-csv fetch-csv]])

  (fetch-csv "variable_min_max.csv")

  (require '[clojure.data.csv :as csv])
  (def vars-w-min-max (parse-csv ))
  (def text (slurp "variable_min_max.csv"))

  (defn parse-csv-to-maps [csv-content]
    (let [rows      (with-open [reader (io/reader csv-content)]
                 (doall
                  (csv/read-csv reader)))
          headers   (map keyword (first rows))
          data-rows (rest rows)]
      (mapv #(zipmap headers %) data-rows)))

  (def variables-w-min-max (parse-csv-to-maps "variable_min_max.csv"))

  (def vars-after-matches
    (map (fn [{:keys [variable] :as m}]
           (assoc m :matches (n-matches 3 variable vars))) variables-w-min-max))

  (defn first-match-key [v]
    (-> v
        (:matches)
        (->> (map last))))

  (defn best-match-key [v]
    (-> v
        (:matches)
        (->> (map last))
        (->> (sort-by #(- 1 (jaro (:variable v) %))))
        (first)))

  (def vars-w-best-match 
    (->> vars-after-matches
         (map #(assoc % :key (best-match-key %)))))

  (defn write-maps-to-csv [maps csv-file]
    (let [headers (map name (keys (first maps)))
          rows    (map (fn [m] (map #(get m %) (keys (first maps)))) maps)]
      (with-open [writer (io/writer csv-file)]
        (csv/write-csv writer (cons headers rows)))))

  ;; Example usage:
  (let [data (->> vars-w-best-match
                  (map #(dissoc % :matches)))
        output-file "variable_min_max_with_keys.csv"]
    (write-maps-to-csv data output-file))

  (best-match-key (first vars-after-matches))

  (sort-by #(- 1 (jaro (:variable (first vars-after-matches)) %))
           (first-match-key (first vars-after-matches)))

  (defn- numeric? [n]
    (re-matches #"^[0-9].*" n))

  (let [[header & rows] (str/split text #"\n")
        header          (str/split header #",")]
    (->> (mapv
          (fn [row]
            [row (count (str/split row #","))]

            #_(into {} (map-indexed (fn [i col]
                                      [(nth header i)
                                       (if (numeric? col) (parse-double col) col)])
                                    (str/split row #","))))
          rows)
         (filter #(< 4 (last %)))))
  vars-w-min-max

  (jaro (-> vars first) "1-h Fuel Load")

  (def var-w-min-max-keys 
    (parse-csv-to-maps "variable_min_max_with_keys.csv"))

  (defn find-var [db k]
    (d/q '[:find ?e .
           :in $ ?k
           :where [?e :variable/bp6-code ?k]]
         db k))

  (d/q '[:find ?e .
         :in $ ?k
         :where [?e :variable/bp6-code ?k]]
       db )

  (find-var db (:key (first var-w-min-max-keys)))

  (:variable/minimum (first entities))

  (def tx (map (fn [{:keys [key min max]}]
                 (-> {:db/id (find-var db key)}        
                     (assoc :variable/maximum (parse-double max))
                     (assoc :variable/minimum (parse-double min))))
               var-w-min-max-keys))

  (d/transact conn tx)



  (map (-> %
           (:key)
           (find-var)):key var-w-min-max-keys)


  )
