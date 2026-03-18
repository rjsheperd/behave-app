(ns posh.reagent
  (:require-macros [reagent.ratom :refer [reaction]])
  (:require [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.impl-rust :as impl-rust]
            [posh.plugin-base :as base
             :include-macros true]
            [reagent.core :as r]
            [reagent.ratom :as ra]))

(def dcfg
  (let [dcfg {:db            d/db
              :pull*         impl-rust/pull
              :pull-many     impl-rust/pull-many
              :q             impl-rust/q
              :filter        d/filter
              :with          d/with
              :entid         impl-rust/entid
              :transact!     d/transact!
              :listen!       d/listen!
              :conn?         d/conn?
              :ratom         r/atom
              :make-reaction ra/make-reaction}]
    (assoc dcfg :pull (partial base/safe-pull dcfg))))

(base/add-plugin dcfg)
