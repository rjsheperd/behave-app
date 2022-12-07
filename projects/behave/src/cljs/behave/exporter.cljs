(ns behave.exporter
  (:require [ajax.core :refer [GET POST]]))

;;; Helpers

(defn- get-content [elem-id]
  (.-innerHTML (js/document.getElementById elem-id)))

;;; Public Fns

(defn export-pdf!
  "Exports an HTML element as a PDF document."
  [elem-id]
  (POST "/pdf" {:params {:html (get-content elem-id)}}))
