(ns behave.solver.worker-client)

(defonce worker (atom nil))

(defn on-message [e]
  (js/console.log e))

(defn post-message [m]
  (.postMessage @worker m))

(defn register-worker []
  (if (.-Worker js/window)
    (do 
      (reset! worker (js/Worker. "/cljs-worker/worker.js"))
      (.addEventListener @worker "message" on-message))
    (prn "worker not supported")))

(comment 
  (register-worker)
  @worker
  (post-message "ping")
  )
