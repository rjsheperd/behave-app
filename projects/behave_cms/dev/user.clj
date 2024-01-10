(ns user
  (:require [shadow.cljs.devtools.api :as shadow]
            [shadow.cljs.devtools.server :as server]
            [behave-cms.server :refer [init-datahike!]]
            [behave-cms.handler :refer [create-handler-stack]]
            [behave.server.interface :refer [start-server!]]
            [behave.config.interface :refer [get-config]]))

(defn cljs-repl
  "Connects to a given build-id. Defaults to `:app`."
  ([]
   (cljs-repl :app))
  ([build-id]
   (println "Starting Behave CMS on: " (get-config :server :http-port))
   (init-datahike!)
   (start-server! {:port    (get-config :server :http-port)
                   :handler (create-handler-stack false false)})
   (server/start!)
   (shadow/watch build-id)
   (shadow/nrepl-select build-id)))

(comment 
  (cljs-repl))
