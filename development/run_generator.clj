(ns run-generator
  (:require [clojure.string :as str]
            [datahike.core :as d]
            [datom-store.main    :as s]
            [datom-utils.interface :refer [safe-deref unwrap]]
            [behave.schema.rules :refer [rules]]
            [string-utils.interface :as su]))

(comment
  (remove-ns 'run-generator)

  (safe-deref s/conn)
  (unwrap s/conn)

  ;; Output
  (d/q '[:find ?s-name ?g-name ?v-name ?v-code ?c-name ?f-name
         :in $ %
         :where
         [?e :module/name "Surface"]
         [?e :module/submodules ?s]

         ;; Submodules
         [?s :submodule/io :output]
         [?s :submodule/name ?s-name]

         ;; Groups
         [?s :submodule/groups ?g]
         [?g :group/name ?g-name]

         ;; Group Variables
         [?g :group/group-variables ?gv]

         ;; Variable
         [?v :variable/group-variables ?gv]
         [?v :variable/name ?v-name]
         (or [?v :variable/bp6-code ?v-code]
             [(ground "CODE-NOT-FOUND") ?v-code])

         ;; CPP Lookup
         [?gv :group-variable/cpp-class ?c-uuid]
         [?gv :group-variable/cpp-function ?f-uuid]

         ;; Class
         [?c :bp/uuid ?c-uuid]
         [?c :cpp.class/name ?c-name]

         ;; Function
         [?f :bp/uuid ?f-uuid]
         [?f :cpp.function/name ?f-name]]
       (safe-deref s/conn) rules)


  ;; Inputs
  (d/q '[:find ?s-name ?g-name ?v-name ?v-code ?c-name ?f-name
         :in $ %
         :where
         [?e :module/name "Surface"]
         [?e :module/submodules ?s]

         ;; Submodules
         [?s :submodule/io :output]
         [?s :submodule/name ?s-name]

         ;; Groups
         (group ?s ?g)
         [?g :group/name ?g-name]

         ;; Group Variables
         (variable ?g ?gv)

         ;; Variable
         [?v :variable/group-variables ?gv]
         [?v :variable/name ?v-name]
         (or [?v :variable/bp6-code ?v-code]
             [(ground "CODE-NOT-FOUND") ?v-code])

         ;; CPP Lookup
         [?gv :group-variable/cpp-class ?c-uuid]
         [?gv :group-variable/cpp-function ?f-uuid]

         ;; Class
         [?c :bp/uuid ?c-uuid]
         [?c :cpp.class/name ?c-name]

         ;; Function
         [?f :bp/uuid ?f-uuid]
         [?f :cpp.function/name ?f-name]
         ]
       (safe-deref s/conn) rules)
  (unwrap s/conn)

  (def resolver-fns {[:continuous true 1] "addGetter"
                     [:discrete true 1]  "addGetter"
                     [:continuous false 1] "addContinuousSetter_1"
                     [:continuous false 2] "addContinuousSetter_2"
                     [:discrete false 1]  "addDiscreteSetter"})

  (def fn-refs 
    (d/q '[:find ?kind ?bp6-code ?c-name ?f-name (count ?p)
           :where
           [?gv :group-variable/cpp-class ?c-uuid]
           [?gv :group-variable/cpp-function ?f-uuid]
           [?v :variable/group-variables ?gv]
           [?v :variable/bp6-code ?bp6-code]
           [?v :variable/kind ?kind]
           [?c :bp/uuid ?c-uuid]
           [?c :cpp.class/name ?c-name]
           [?f :bp/uuid ?f-uuid]
           [?f :cpp.function/name ?f-name]
           [?f :cpp.function/parameter ?p]]
         @@s/conn))

  (def cpp-resolvers
    (map (fn [[kind bp6-code c-name f-name p-count]]
           (let [getter? (str/starts-with? f-name "get")]
             (println c-name f-name kind getter? p-count (resolver-fns [kind getter? p-count]))
             (format "%s(\"%s\", &%s::%s);"
                     (resolver-fns [kind getter? p-count])
                     bp6-code
                     c-name
                     f-name)))
         (sort-by #(nth % 2) fn-refs)))

  (spit "funcs.cpp" (str/join "\n" cpp-resolvers))

  ;; Update vars without codes
  (def fns-wo-codes
    (d/q '[:find ?v ?v-name
           :where
           [?gv :group-variable/cpp-class ?c-uuid]
           [?gv :group-variable/cpp-function ?f-uuid]
           [?v :variable/group-variables ?gv]
           [?v :variable/name ?v-name]
           (not [?v :variable/bp6-code ?bp6-code])
           [?v :variable/kind ?kind]
           [?c :bp/uuid ?c-uuid]
           [?c :cpp.class/name ?c-name]
           [?f :bp/uuid ?f-uuid]
           [?f :cpp.function/name ?f-name]
           [?f :cpp.function/parameter ?p]]
         @@s/conn))

  (def add-codes-tx (map (fn [[eid v-name]]
                           {:db/id             eid
                            :variable/bp6-code (su/->code v-name)})
                         fns-wo-codes))

  (d/transact (unwrap s/conn) add-codes-tx)


  (let [lines         (str/split-lines (slurp "var_fns_table.tsv"))
        [header & rows] (map #(str/split % #"\s+\\t\s+") lines)
        header        (map keyword header)]
    (println header (count rows) (first (map str/trim (first rows)))))
    (map (fn [row] (into {} (map vector header row))) rows))

  (defn var-lookup-by-fn [fn-name]
    (d/q '[:find ?sm-name ?g-name;; ?v-name
           :in $ % ?f-name
           :where
           [?f :cpp.function/name ?f-name]
           [?f :bp/uuid ?f-uuid]
           [?gv :group-variable/cpp-function ?f-uuid]
           [variable ?gv ?v]
           [?v :variable/name ?v-name]
           [group-variable ?g ?gv]
           [?g :group/name ?g-name]
           [group ?sm ?g]
           [?sm :submodule/name ?sm-name]
           #_[submodule ?m ?sm]]
         @@s/conn rules fn-name))

  (def f-name "getCharacteristicMoistureByLifeState")
  (var-lookup-by-fn f-name)


  )
