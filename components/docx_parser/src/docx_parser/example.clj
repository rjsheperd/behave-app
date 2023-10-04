(ns docx-parser.example
  (:require [clojure.java.io :as io]
            [clojure.string :as str])
  (:import [org.docx4j Docx4J Docx4jProperties]
           [org.docx4j.convert.out ConversionFeatures HTMLSettings]
           [org.docx4j.convert.out.html SdtWriter SdtToListSdtTagHandler]
           [org.docx4j.fonts BestMatchingMapper IdentityPlusMapper Mapper PhysicalFont PhysicalFonts]
           [org.docx4j.model.fields FieldUpdater]
           [org.docx4j.openpackaging.packages WordprocessingMLPackage]))


(require '[clojure.reflect :as refl])


(defn display-public-methods [class-name]
  (let [class (Class/forName class-name)]
        methods (.getMethods class)))
    (doseq [method methods
            :let [method-name (str method)]]
      (println method-name))))



(def ^:private save true)
(def ^:private nest-lists true)

(defn to-html [input-file output-file]
  (let [word-ml-package (Docx4J/load (io/file input-file))

        ;; CSS reset
        user-css (if nest-lists
                   "html, body, div, span, h1, h2, h3, h4, h5, h6, p, a, img, table, caption, tbody, tfoot, thead, tr, th, td { margin: 0; padding: 0; border: 0; } body { line-height: 1; }"
                   "html, body, div, span, h1, h2, h3, h4, h5, h6, p, a, img, ol, ul, li, table, caption, tbody, tfoot, thead, tr, th, td { margin: 0; padding: 0; border: 0; } body { line-height: 1; }")
        
        ;; HTML exporter setup (required)
        html-settings (Docx4J/createHTMLSettings)

        image-dir-path (str (subs input-file 0 (str/index-of input-file ".docx")) "_files")]

    (doto html-settings 
      (.setImageDirPath image-dir-path)
      (.setImageTargetUri image-dir-path)
      (.setWmlPackage word-ml-package)
      (.setUserCSS user-css))
    
    ;; List numbering
    (if nest-lists
      (SdtWriter/registerTagHandler "HTML_ELEMENT" (SdtToListSdtTagHandler.))
      (.remove (.-features html-settings) ConversionFeatures/PP_HTML_COLLECT_LISTS))
            
    ;; Refresh the values of DOCPROPERTY fields
    (when-let [updater (FieldUpdater. word-ml-package)]
      (.update updater true))
    
    ;; Output to an OutputStream
    (let [os (io/output-stream output-file)]
      
      ;; If you want XHTML output
      (Docx4jProperties/setProperty "docx4j.Convert.Out.HTML.OutputMethodXML" true)
      
      (Docx4J/toHTML html-settings os Docx4J/FLAG_EXPORT_PREFER_XSL)
      
      (if save
        (println "Saved: " output-file)
        (println (.toString os)))
      
      ;; Clean up
      (when (.getFontTablePart (.getMainDocumentPart word-ml-package))
        (.deleteEmbeddedFontTempFiles (.getFontTablePart (.getMainDocumentPart word-ml-package)))))))

(to-html "CEO_Manual.docx" "ceo_manual.html")
(def input-file "CEO_Manual.docx")

*1
