(ns behave.test-runner
  (:require [clojure.string :as str]
            [cljs-test-display.core]
            [figwheel.main.testing :refer [run-tests]]
            [behave.vms.store :refer [load-vms!]]
            [behave.contain-test :as contain-test]))

(defn run-the-tests []
  (println "RUNNING TESTS")
  (run-tests (cljs-test-display.core/init! "app-testing")))

(defn add-script [js-path]
  (let [script-el (.createElement js/document "script")]
    (set! (.-src script-el) js-path)
    (set! (.-type script-el) "text/javascript")
    (-> js/document
        (.-body)
        (.appendChild script-el))))

(defn ^:after-load init []
  (println "LOADED THE INIT")
  (let [window-keys (js->clj (.keys js/Object js/window))
        module-loaded? (seq (filter #(str/starts-with? % "Module") window-keys))]
    (if-not module-loaded?
      (do (add-script "/js/behave.js")
          (load-vms!)
          (js/setTimeout #(run-the-tests) 100))
      (run-the-tests))))

(init)
