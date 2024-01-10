(ns user
  (:require [shadow.cljs.devtools.api :as shadow]
            [shadow.cljs.devtools.server :as server]
            [behave.core             :refer [init! development-app]]
            [behave.server.interface :refer [start-server!]]
            [behave.config.interface :refer [get-config]]))

(defn cljs-repl
  "Connects to a given build-id. Defaults to `:app`."
  ([]
   (cljs-repl :app))
  ([build-id]
   (println "Starting Behave Dev App")
   (get-config :server :http-port)
   (init!)
   (start-server! {:port 8003 :handler development-app})
   (server/start!)
   (shadow/watch build-id)
   (shadow/nrepl-select build-id)))

(comment 
  (cljs-repl))
