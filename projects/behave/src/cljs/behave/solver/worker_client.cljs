(ns behave.solver.worker-client)

(defn register-service-worker []
  (if (.-serviceWorker js/navigator)
    (-> (js/navigator.serviceWorker.register "/js-worker/service-worker.js")
        (.then #(.log js/console (str "Service worker registered " %)) )
        (.catch #(.log js/console (str "Service worker failed " %))))
    (prn "service worker not supported")))

(register-service-worker)

(.log js/console "demo webworker")
