(ns behave.contain-test
  (:require [clojure.core.async   :refer [go <!]]
            [cljs.test            :refer [is deftest testing]]
            [csv-parser.interface :refer [fetch-csv parse-csv]]
            [behave.lib.contain   :as contain]
            [behave.lib.enums     :as enums]
            [behave.lib.units     :refer [get-unit]]))

;; Helpers

(defn within? [precision a b]
  (> precision (- a b)))

(def within-millionth? (partial within? 1e-06))

;; Tests

(deftest csv-test
  (testing "CSV file is fetched and parsed"
    (go
      (let [csv-text (<! (fetch-csv "/csv/contain.csv"))
            results  (parse-csv csv-text)]
        (is (= 2 (count results)))
        (is (= 17 (count (first results))))))))

(deftest contain-testing-simple
  (go
    (let [row    (->> "/csv/contain-simple.csv"
                      (fetch-csv)
                      (<!)
                      (parse-csv)
                      (first))
          module (contain/init)]

                                        ; Arrange
      (-> module
          (contain/setAttackDistance (get row "attackDistance") (get-unit "ch"))
          (contain/setLwRatio (get row "lwRatio"))
          (contain/setReportRate (get row "reportRate") (get-unit "ch/h"))
          (contain/setReportSize (get row "reportSize") (get-unit "ac"))
          (contain/setTactic (enums/contain-tactic (get row "tactic")))
          (contain/addResource (get row "resourceArrival")
                               (get row "resourceProduction")
                               (get-unit "h")
                               (get row "resourceDuration")
                               (get-unit "ch/h")
                               (get row "resourceDescription")))

                                        ; Act
      (contain/doContainRun module)

                                        ; Assert
      (is (within-millionth? (get row "fireLineLength")           (contain/getFinalFireLineLength module (get-unit "ch"))))
      (is (within-millionth? (get row "perimeterAtInitialAttack") (contain/getPerimeterAtInitialAttack module (get-unit "ch"))))
      (is (within-millionth? (get row "perimeterAtContainment")   (contain/getPerimeterAtContainment module (get-unit "ch"))))
      (is (within-millionth? (get row "fireSizeAtInitialAttack")  (contain/getFireSizeAtInitialAttack module (get-unit "ac"))))
      (is (within-millionth? (get row "fireSize")                 (contain/getFinalFireSize module (get-unit "ac"))))
      (is (within-millionth? (get row "containmentArea")          (contain/getFinalContainmentArea module (get-unit "ac"))))
      (is (within-millionth? (get row "timeSinceReport")          (contain/getFinalTimeSinceReport module (get-unit "m"))))
      (is (= (enums/contain-status (get row "containmentStatus")) (contain/getContainmentStatus module))))))
