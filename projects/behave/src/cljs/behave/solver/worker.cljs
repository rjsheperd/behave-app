(ns behave.solver.worker)

(set!
 (.-onmessage js/self) 
 (fn [event]
   (js/console.log event)
   (.postMessage js/self "pong!")))
