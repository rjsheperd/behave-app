(ns absurder-sql.test-preloads)

;; Intercept console.error and echo via println so kaocha-cljs2 captures it
;; in its test output (it hooks *print-fn*).
(defonce ^:private _install-console-capture
  (let [orig-error js/console.error]
    (set! js/console.error
          (fn [& args]
            (apply orig-error args)
            (println ">>> CONSOLE.ERROR:" (apply str (interpose " " (map str args))))))
    true))
