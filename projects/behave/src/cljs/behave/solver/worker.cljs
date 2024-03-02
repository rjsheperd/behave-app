(ns behave.solver.worker)

(set!
 (.-onmessage js/self) 
 (fn [event]
   (js/console.log event)
   (js/console.log (js/Module.SIGContainAdapter.))
   (.postMessage js/self "pong!")))

(js/importScripts "/js/behave-min.js")
