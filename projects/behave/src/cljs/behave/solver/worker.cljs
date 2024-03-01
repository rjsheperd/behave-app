(ns behave.solver.worker)

(.addEventListener js/self "install" (fn [event] (prn (str "service worker installed"))))
