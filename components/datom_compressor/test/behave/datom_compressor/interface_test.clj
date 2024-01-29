(ns behave.datom-compressor.interface-test
  (:require
    [clojure.test :refer [deftest is]]
    [behave.datom-compressor.interface :as compress]))

;;; Data

(def ^:private test-datoms [[1 :name "Alice" 30000 true]
                            [1 :age  25      30000 true]
                            [2 :name "Alice" 30000 true]
                            [2 :age  25      30000 true]])

(defn- sort-datoms [datoms]
  (sort-by (juxt first second) datoms))

;;; Tests

(deftest test-compress
  (let [roundtrip (compress/unpack (compress/pack test-datoms))]
    (is (= (sort-datoms test-datoms) (sort-datoms roundtrip)))))
