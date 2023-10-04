(ns docx-parser.core
  (:require [clojure.java.io :as io])
  (:import [org.docx4j.openpackaging.packages WordprocessingMLPackage]
           [org.docx4j.openpackaging.exceptions Docx4JException]
           [javax.xml.bind JAXBElement]))

(defn parse-openxml [file]
  (try
    (let [pkg (WordprocessingMLPackage/load file)]
      (-> (.getMainDocumentPart pkg)
          (.getJaxbElement)))
    (catch Docx4JException e
      (println "Error parsing OpenXML: " (.getMessage e)))))

;; Example usage
(def openxml-file "CEO_Manual.docx")
(.exists (io/file openxml-file))

;; Parse the OpenXML document
(def parsed-doc (parse-openxml (io/file openxml-file)))

;; Process the parsed-doc as needed
(when parsed-doc
  (println "Parsed OpenXML document:" parsed-doc))

(.getContent parsed-doc)
