(ns repl
   (:require [shadow.cljs.devtools.api :as shadow]
             [shadow.cljs.devtools.server :as server]
             [behave.server.interface :as s]
             [behave.core :as c]))

(defn cljs-repl
  "Connects to a given build-id. Defaults to `:app`."
  ([]
   (cljs-repl :app))
  ([build-id]
   (server/start!)
   (shadow/watch build-id)
   (shadow/nrepl-select build-id)))

(comment 
  (s/start-server! c/development-app)

  (cljs-repl)

  )
