(ns behave.export-pdf
  (:require [clj-htmltopdf.core  :refer [->pdf]])
  (:import [java.util UUID]))

(defn pdf-handler [{:keys [params]}]
  (let [filename (str (UUID/randomUUID) ".pdf")]
    (->pdf (:html params) filename)
    {:status 200 :body filename}))
