(ns behave.worksheet-test
  (:require [cljs.test   :refer [is deftest testing] :refer-macros [use-fixtures]]
            [clojure.set :as set]
            [datascript.core :as d]
            [re-frame.core :as rf]
            [re-frisk.core :as re-frisk]
            [behave.store :refer [load-store! conn]]
            [behave.events]
            [behave.subs]))

;; Constants

(def fs->bp6 {; Inputs
              :fs/ReportSize                         "vContainReportSize"
              :fs/LineConstructionOffset             "vContainAttackDist"
              :fs/LengthToWidthRatio                 "vContainReportRatio"
              :fs/SurfaceFireRateOfSpread            "vContainReportSpread"
              :fs/SuppressionTactic                  "vContainAttackTactic"
              :fs/ResourceArrivalTime                "vContainResourceArrival"
              :fs/ResourceDuration                   "vContainResourceDuration"
              :fs/ResourceProductionRate             "vContainResourceProd"
              :fs/ResourceName                       "vContainResourceName"

            ; Outputs
              :fs/FirelineConstructed                "vContainLine"
              :fs/FireAreaAtResourceArrivalTime      "vContainAttackSize"
              :fs/FirePerimeterAtResourceArrivalTime "vContainAttackPerimeter"
              :fs/ContainedArea                      "vContainSize"
              :fs/TimeFromReport                     "vContainTime"
              :fs/ContainStatus                      "vContainStatus"})

;; Fixtures

(defn before-tests [fn]
  (load-store!)
  (re-frisk/enable)
  (fn))

(use-fixtures :once before-tests)

;; Tests

(deftest new-worksheet
  ;; Arrange
  (let [ws-uuid (str (d/squuid))
        name    "Test Worksheet"
        modules #{:surface :contain}]

    ;; Act
    (rf/dispatch-sync [:worksheet/new {:uuid    ws-uuid
                                       :name    name
                                       :modules modules}])

    ;; Assert
    (is (= 1       (count (d/q '[:find [?e ...] :in $ ?ws-uuid :where [?e :worksheet/uuid ?ws-uuid]] @@conn ws-uuid))))
    (is (= modules (set (d/q '[:find [?m ...]
                               :in $ ?ws-uuid
                               :where [?e :worksheet/uuid ?ws-uuid]
                                      [?e :worksheet/modules ?m]] @@conn ws-uuid))))
    (is (= name    (first (d/q '[:find [?n]
                                 :in $ ?ws-uuid
                                 :where [?e :worksheet/uuid ?ws-uuid]
                                        [?e :worksheet/name ?n]] @@conn ws-uuid))))))

(deftest worksheet-inputs
  ;; Arrange
  (let [ws-uuid (str (d/squuid))
        name    "Test Worksheet"
        modules #{:surface}
        input   {:group "123" :id 0 :var "abc" :value "50" :units "h"}]

    ;; Act
    (rf/dispatch-sync [:worksheet/new {:uuid    ws-uuid
                                       :name    name
                                       :modules modules}])

    (rf/dispatch-sync [:worksheet/add-input-group
                       ws-uuid
                       (:group input)
                       (:id    input)])

    
    (rf/dispatch-sync [:worksheet/upsert-input-variable
                       ws-uuid
                       (:group input)
                       (:id    input)
                       (:var   input)
                       (:value input)
                       (:units input)])

    ;; Assert
    (is (= 1 (count (d/q '[:find  [?ig ...]
                           :in    $ ?ws-uuid
                           :where [?e :worksheet/uuid ?ws-uuid]
                                  [?e :worksheet/input-groups ?ig]] @@conn ws-uuid))))

    (is (= 1 (count (d/q '[:find  [?i ...]
                           :in    $ ?ws-uuid ?g-uuid ?repeat-id
                           :where [?e :worksheet/uuid ?ws-uuid]
                                  [?e :worksheet/input-groups ?ig]
                                  [?ig :input-group/group-uuid ?g-uuid]
                                  [?ig :input-group/repeat-id ?repeat-id]
                                  [?ig :input-group/inputs ?i]]
                         @@conn ws-uuid (:group input) (:id input)))))

    (is (= (:value input) (first (d/q '[:find  [?v ...]
                                        :in    $ ?ws-uuid ?g-uuid ?repeat-id
                                        :where [?e :worksheet/uuid ?ws-uuid]
                                               [?e :worksheet/input-groups ?ig]
                                               [?ig :input-group/group-uuid ?g-uuid]
                                               [?ig :input-group/repeat-id ?repeat-id]
                                               [?ig :input-group/inputs ?i]
                                               [?i :input/value ?v]]
                                      @@conn ws-uuid (:group input) (:id input)))))

    (is (= (:units input) (first (d/q '[:find  [?u ...]
                                        :in    $ ?ws-uuid ?g-uuid ?repeat-id
                                        :where [?e :worksheet/uuid ?ws-uuid]
                                               [?e :worksheet/input-groups ?ig]
                                               [?ig :input-group/group-uuid ?g-uuid]
                                               [?ig :input-group/repeat-id ?repeat-id]
                                               [?ig :input-group/inputs ?i]
                                               [?i :input/units ?u]]
                                      @@conn ws-uuid (:group input) (:id input)))))))
    
(deftest worksheet-outputs
  ;; Arrange
  (let [ws-uuid (str (d/squuid))
        name    "Test Worksheet"
        modules #{:surface}
        output  "abc"]

    ;; Act
    (rf/dispatch-sync [:worksheet/new {:uuid    ws-uuid
                                       :name    name
                                       :modules modules}])

    (rf/dispatch-sync [:worksheet/upsert-output ws-uuid output true])

    ;; Assert
    (is (= 1 (count (d/q '[:find  [?o ...]
                           :in    $ ?ws-uuid
                           :where [?e :worksheet/uuid ?ws-uuid]
                                  [?e :worksheet/outputs ?o]] @@conn ws-uuid))))

    (is (= 1 (count (d/q '[:find  [?o ...]
                           :in    $ ?ws-uuid ?g-uuid
                           :where [?e :worksheet/uuid ?ws-uuid]
                                  [?e :worksheet/outputs ?o]
                                  [?o :output/group-variable-uuid ?g-uuid]] @@conn ws-uuid output))))

    (is (= true (first (d/q '[:find  [?enabled ...]
                              :in    $ ?ws-uuid ?g-uuid
                              :where [?e :worksheet/uuid ?ws-uuid]
                                     [?e :worksheet/outputs ?o]
                                     [?o :output/group-variable-uuid ?g-uuid]
                                     [?o :output/enabled? ?enabled]]
                            @@conn ws-uuid output))))))

(deftest worksheet-result-table
  ;; Arrange
  (let [ws-uuid (str (d/squuid))
        name     "Test Worksheet"
        modules  #{:surface}
        variable "abc"
        value    "30"
        units    "h"]

    (testing "Creating a result table"
      ;; Act
      (rf/dispatch-sync [:worksheet/new {:uuid    ws-uuid
                                         :name    name
                                         :modules modules}])

      (rf/dispatch-sync [:worksheet/add-result-table ws-uuid])

      ;; Assert
      (is (= 1 (count (d/q '[:find  [?t ...]
                             :in    $ ?ws-uuid
                             :where [?e :worksheet/uuid ?ws-uuid]
                                     [?e :worksheet/result-table ?t]] @@conn ws-uuid)))))

    (testing "Creating a result table header"
      ;; Act
      (rf/dispatch-sync [:worksheet/add-result-table-header ws-uuid variable units])

      ;; Assert
      (is (= 1 (count (d/q '[:find  [?h ...]
                             :in    $ ?ws-uuid
                             :where [?e :worksheet/uuid ?ws-uuid]
                                    [?e :worksheet/result-table ?t]
                                    [?t :result-table/headers ?h]] @@conn ws-uuid)))))

      (is (= units (first (d/q '[:find  [?u ...]
                                :in    $ ?ws-uuid ?v-uuid
                                :where [?e :worksheet/uuid ?ws-uuid]
                                       [?e :worksheet/result-table ?t]
                                       [?t :result-table/headers ?h]
                                       [?h :result-header/group-variable-uuid ?v-uuid]
                                       [?h :result-header/units ?u]]
                             @@conn ws-uuid variable))))

    (testing "Creating a result table row"
      ;; Act
      (rf/dispatch-sync [:worksheet/add-result-table-row ws-uuid 0])

      ;; Assert
      (is (= 1 (count (d/q '[:find  [?r ...]
                             :in    $ ?ws-uuid
                             :where [?e :worksheet/uuid ?ws-uuid]
                                    [?e :worksheet/result-table ?t]
                                    [?t :result-table/rows ?r]] @@conn ws-uuid))))

      (is (= 0 (first (d/q '[:find  [?id ...]
                             :in    $ ?ws-uuid
                             :where [?e :worksheet/uuid ?ws-uuid]
                                    [?e :worksheet/result-table ?t]
                                    [?t :result-table/rows ?r]
                                    [?r :result-row/id ?id]] @@conn ws-uuid)))))

    (testing "Creating a result table cell"
      ;; Act
      (rf/dispatch-sync [:worksheet/add-result-table-cell ws-uuid 0 variable value units])

      ;; Assert
      (is (= 1 (count (d/q '[:find  [?r ...]
                             :in    $ ?ws-uuid
                             :where [?e :worksheet/uuid ?ws-uuid]
                                    [?e :worksheet/result-table ?t]
                                    [?t :result-table/rows ?r]
                                    [?r :result-row/cells ?c]] @@conn ws-uuid))))

      (is (= value (first (d/q '[:find  [?v ...]
                                 :in    $ ?ws-uuid ?v-uuid
                                 :where [?e :worksheet/uuid ?ws-uuid]
                                        [?e :worksheet/result-table ?t]
                                        [?t :result-table/headers ?h]
                                        [?h :result-header/group-variable-uuid ?v-uuid]
                                        [?t :result-table/rows ?r]
                                        [?r :result-row/cells ?c]
                                        [?c :result-cell/header ?h]
                                        [?c :result-cell/value ?v]] @@conn ws-uuid variable)))))))


(comment

  (def ws-uuid "63759501-c1be-4287-a9e0-74540fa8002c")
  (def variable "abc")

  (rf/subscribe [:worksheet/results-table-headers ws-uuid])

  (rf/subscribe [:worksheet/results-table-rows ws-uuid])

  (rf/subscribe [:worksheet/results-table-cells ws-uuid 0])

  (rf/subscribe [:worksheet/results-table-column ws-uuid variable])

  (def table (first (d/q '[:find  [?t ...]
                           :in    $ ?ws-uuid
                           :where [?e :worksheet/uuid ?ws-uuid]
                                  [?e :worksheet/result-table ?t]] @@conn ws-uuid)))

  table

  (def row (first (d/q '[:find  [?r ...]
                         :in    $ ?t ?row-id
                         :where [?t :result-table/rows ?r]
                                [?r :result-row/id ?row-id]] @@conn table 0)))

  row

  (def header (first (d/q '[:find  [?h ...]
                            :in    $ ?t ?group-var-uuid
                            :where [?t :result-table/headers ?h]
                                   [?h :result-header/group-variable-uuid ?group-var-uuid]]

                          @@conn table variable)))

  header

  (def value "50")

  (d/transact @conn [{:db/id row
                      :result-row/cells [{:result-cell/header header
                                          :result-cell/value  value}]}])

  (d/q '[:find  [?c ...]
         :in    $ ?ws-uuid
         :where [?e :worksheet/uuid ?ws-uuid]
         [?e :worksheet/result-table ?t]
         [?t :result-table/rows ?r]
         [?r :result-row/cells ?c]] @@conn ws-uuid)

  (d/q '[:find  [?v ...]
         :in    $ ?ws-uuid ?v-uuid
         :where [?e :worksheet/uuid ?ws-uuid]
                [?e :worksheet/result-table ?t]
                [?t :result-table/headers ?h]
                [?h :result-header/group-variable-uuid ?v-uuid]
                [?t :result-table/rows ?r]
                [?r :result-row/cells ?c]
                [?c :result-cell/header ?h]
                [?c :result-cell/value ?v]] @@conn ws-uuid variable)

  )
