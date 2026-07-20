((nil . ((cider-clojure-cli-aliases . ":dev:behave/app")
         (cider-default-cljs-repl . figwheel-main)))

 ("projects/behave" . ((nil . ((cider-preferred-build-tool . shadow-cljs)
                               (cider-default-cljs-repl . shadow)
                               (cider-shadow-default-options . "browser")
                               (cider-shadow-cljs-command . "bun shadow-cljs")
                               (cider-clojure-cli-aliases . nil))))))

;; VMS Configuration
;; TODO: Fix to avoid having two separate aliases for projects
;; ((nil . ((cider-clojure-cli-aliases . "-A:dev:behave/vms")
;;          (cider-default-cljs-repl . figwheel-main)
;;          (cider-figwheel-main-default-options . "vms"))))
