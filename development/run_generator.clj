(ns run-generator
  (:require [datascript.core     :as d]
            [datom-store.main    :as s]
            [behave.schema.rules :refer [rules]]))


(comment

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
         [?f :cpp.function/name ?f-name]
         ]
       @s/conn rules)


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
       @@vms/vms-conn rules)

  )
