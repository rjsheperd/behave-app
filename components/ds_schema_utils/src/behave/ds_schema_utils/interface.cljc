(ns behave.ds-schema-utils.interface
  (:require [behave.ds-schema-utils.core :as c]))

(def ^{:argslist '([datomic-schema])
       :doc "Transforms a Datomic-style schema to DataScript."}
  ->ds-schema c/->ds-schema)
