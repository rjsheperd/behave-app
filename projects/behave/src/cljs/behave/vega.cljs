(ns behave.vega
  (:require [cljs.core.async.interop :refer-macros [<p!]]
            [cljsjs.vega-embed]
            [clojure.core.async      :refer [go]]
            [reagent.core            :as r]
            [reagent.dom             :as rd]))

;;; Helper Fns

(defn- render-vega [spec on-click elem]
  (when (and spec (seq (get-in spec [:data :values])))
    (go
      (try
        (let [result (<p! (js/vegaEmbed elem (clj->js spec) #js {:renderer "svg"}))]
          (-> result .-view (.addEventListener "click" on-click)))
        (catch ExceptionInfo e (js/console.log (ex-cause e)))))))

(defn- vega-canvas []
  (r/create-class
   {:component-did-mount
    (fn [this]
      (let [{:keys [spec on-click]} (r/props this)]
        (render-vega spec on-click (rd/dom-node this))))

    :component-did-update
    (fn [this _]
      (let [{:keys [spec on-click]} (r/props this)]
        (render-vega spec on-click (rd/dom-node this))))

    :render
    (fn [this]
      [:div#vega-canvas
       {:style {:height (:box-height (r/props this))
                :width  (:box-width  (r/props this))}}])}))

(defn- ex-plot []
  {:schema "https://vega.github.io/schema/vega-lite/v5.json"
   :description "A simple bar chart with embedded data."
   :data {:values [{:a "A" :b 28} {:a "B" :b 55} {:a "C" :b 43}
                   {:a "D" :b 91} {:a "E" :b 81} {:a "F" :b 53}
                   {:a "G" :b 19} {:a "H" :b 87} {:a "I" :b 52}]}
   :mark "bar"
   :encoding {:x {:field "a" :type "nominal" :axis {:labelAngle 0}}
              :y {:field "b" :type "quantitative"}}})

;;; UI Components

(defn vega-chart
  "A function to create a Vega line plot."
  [{:keys [box-height box-width on-click]}]
  [vega-canvas {:spec       (ex-plot)
                :box-height box-height
                :box-width  box-width
                :on-click   on-click}])

(comment
  (ex-plot)

  (vega-chart {:box-height 100 :box-width 100}))
