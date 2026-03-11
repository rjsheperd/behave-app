(ns absurder-sql.test-runner
  (:require [absurder-sql.datascript.client-init-test]
            [absurder-sql.interface-test]
            [cljs.test :as t]))

(defmethod t/report [:cljs.test/default :end-run-tests] [m]
  (let [pass     (:pass m)
        fail     (:fail m)
        error    (:error m)
        total    (+ pass fail error)
        success? (zero? (+ fail error))]
    (println (str "\n" total " assertions, " pass " passed, " fail " failed, " error " errors"))
    (set! (.-__test_exit_code js/window) (if success? 0 1))
    (set! (.-__test_done js/window) true)))

(defn ^:export init []
  (t/run-tests 'absurder-sql.interface-test
               'absurder-sql.datascript.client-init-test))
