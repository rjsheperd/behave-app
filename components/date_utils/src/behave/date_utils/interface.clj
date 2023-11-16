(ns behave.date-utils.interface
  (:require [behave.date-utils.core :as c]))

(def ^{:argslist '()
       :doc "Today's date in string format 'yyyy-MM-dd'."}
 today c/today)
