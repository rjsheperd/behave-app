(ns behave.translate
  (:require [datascript.core :as d]
            [re-frame.core :refer [subscribe dispatch-sync]]
            [behave.vms.store :refer [vms-conn]]))

;;; Configuration

(def ^:private supported #{"en-US" "pt-PT"})
(def ^:private default "en-US")

;;; Helpers

(defn- get-translations [shortcode]
  (->>
   (d/q '[:find ?key ?translation
          :in $ ?lang-shortcode
          :where
          [?l :language/shortcode ?lang-shortcode]
          [?l :language/translation ?t]
          [?t :translation/key ?key]
          [?t :translation/translation ?translation]]
        @@vms-conn shortcode)
   (into {})))

(defn browser-lang []
  (.. js/window -navigator -language))

;;; Public Fns

(defn bp
  [& s]
  (apply str "behaveplus:" s))

(defn <t
  "Returns the translation for `translation-key`.

  Example:
  ```
  (defn my-component []
    [:btn @(<t \"success\")])
  ```"
  [translation-key]
  (subscribe [:t translation-key]))

(defn load-translations! []
  (let [browser   (browser-lang)
        shortcode (if (contains? supported browser) browser default)]
    (->> (get-translations shortcode)
         (conj [:translations/load shortcode])
         (dispatch-sync))))
