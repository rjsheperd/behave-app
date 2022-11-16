(ns behave.contain-test
  (:require [clojure.string       :as str]
            [clojure.set          :as set]
            [clojure.core.async   :refer [go <!]]
            [cljs.test            :refer [is deftest testing] :refer-macros [use-fixtures]]
            [csv-parser.interface :refer [fetch-csv parse-csv]]
            [re-frame.core        :as rf]
            [re-frisk.core        :as re-frisk]
            [datascript.core      :as d]
            [behave.lib.contain   :as contain]
            [behave.events]
            [behave.subs          :refer [<sub]]
            [behave.vms.store     :refer [load-vms!]]
            [behave.store         :refer [load-store! conn]]
            [behave.vms.subs]))

;; Constants

(def fs->bp6 {; Inputs
            "fs:ReportSize"                         "vContainReportSize"
            "fs:LineConstructionOffset"             "vContainAttackDist"
            "fs:LengthToWidthRatio"                 "vContainReportRatio"
            "fs:SurfaceFireRateOfSpread"            "vContainReportSpread"
            "fs:SuppressionTactic"                  "vContainAttackTactic"
            "fs:ResourceArrivalTime"                "vContainResourceArrival"
            "fs:ResourceDuration"                   "vContainResourceDuration"
            "fs:ResourceProductionRate"             "vContainResourceProd"
            "fs:ResourceName"                       "vContainResourceName"

            ; Outputs
            "fs:FirelineConstructed"                "vContainLine"
            "fs:FireAreaAtResourceArrivalTime"      "vContainAttackSize"
            "fs:FirePerimeterAtResourceArrivalTime" "vContainAttackPerimeter"
            "fs:ContainedArea"                      "vContainSize"
            "fs:TimeFromReport"                     "vContainTime"
            "fs:ContainStatus"                      "vContainStatus"})

(def bp6->fs (set/map-invert fs->bp6))

;; Helpers

(defn var-path-by-bp6-code-name [bp6-code-name]
  (let [[group-uuid gv-uuid]
        (first @(rf/subscribe [:vms/query '[:find ?g-uuid ?gv-uuid
                                            :in $ ?bp6-code-name
                                            :where [?e :variable/bp6-code-name ?bp6-code-name]
                                            [?e :variable/group-variables ?gv]
                                            [?g :group/group-variables ?gv]
                                            [?gv :bp/uuid ?gv-uuid]
                                            [?g :bp/uuid ?g-uuid]]
                               bp6-code-name]))]
    [group-uuid 0 gv-uuid]))

(defn get-input-groups [module]
 (let [*module          @(rf/subscribe [:wizard/*module module])
       input-submodules (filter #(= (:submodule/io %) :input)
                                   @(rf/subscribe [:wizard/submodules (:db/id *module)]))]
   (set (flatten (map #(<sub [:vms/query '[:find [?uuid] :in $ ?e :where [?e :bp/uuid ?uuid]] (:db/id %)]) (flatten (map :submodule/groups input-submodules)))))))

;; Fixtures

(defn before-tests [fn]
  (load-vms!)
  (load-store!)
  (re-frisk/enable)
  (fn))

(use-fixtures :once before-tests)

;; Tests

(deftest csv-test
  (testing "CSV file is fetched and parsed"
    (go
      (let [csv-text (<! (fetch-csv "/csv/contain.csv"))
            results  (parse-csv csv-text)]
        (is (= 2 (count results)))
        (is (= 17 (count (first results))))))))

(def csv-results (atom nil))
(def wip-ws-uuid (atom nil))

(defn xf-csv-keys [results]
  (mapv
   (fn [row]
     (into {}
           (mapv
             (fn [[k v]]
               (if-let [[_ var-name units] (re-matches #"(.*)?\((.*)\)" k)]
                 {var-name {:units units
                            :value v}}
                 {k {:value v}})) row)))
   results))

(deftest csv-to-worksheet
  (go
    (let [results          (-> "/csv/contain.csv"
                               (fetch-csv)
                               (<!)
                               (parse-csv)
                               (xf-csv-keys))
          ws-uuid          (str (d/squuid))
          input-groups     (get-input-groups "contain")
          headers          (keys (first results))
          input-paths      (filter #(->> % second first (contains? input-groups))
                                   (map (fn [k] [k (-> k fs->bp6 var-path-by-bp6-code-name)])
                                        headers))
          first-row        (first results)]

      ; Create worksheet
      (rf/dispatch [:worksheet/new {:uuid ws-uuid
                                    :name "Contain Testing Worksheet"
                                    :modules #{:surface :contain}}])

      ; Load inputs
      #_(map (fn [[k path]] (let [[group-uuid repeat-id group-var-uuid] path
                                input (get first-row k)]
                            (rf/dispatch [:worksheet/add-input-group
                                               ws-uuid
                                               group-uuid
                                               repeat-id])
                            (rf/dispatch [:worksheet/upsert-input-variable
                                          ws-uuid
                                          group-uuid
                                          repeat-id group-var-uuid
                                          (:value input)
                                          (:units input)])))
           input-paths)

      ; 2. Match the input groups to the keys
      (println [:WS_UUID ws-uuid] [:CSV_RESULTS results] [:INPUT-PATHS input-paths])
      (reset! wip-ws-uuid ws-uuid)
      (reset! csv-results results))))


(comment
  @csv-results
  @wip-ws-uuid

  #_(into {} (first @csv-results))
  (rf/subscribe [:query
                 '[:find ?e
                   :in $ ?uuid
                   :where [?e :worksheet/uuid ?uuid]]
                 [@wip-ws-uuid]])
  (rf/subscribe [:pull '[*] 299])

  (rf/subscribe [:worksheet/latest])

  (def input-groups (get-input-groups "contain"))
  (def headers (keys (first @csv-results)))
  headers
  (def input-paths (filter #(->> % second first (contains? input-groups)) (map (fn [k] [k (-> k fs->bp6 var-path-by-bp6-code-name)]) headers)))

  input-paths
  (def first-row (first @csv-results))

  (map (fn [[k path]] (let [[group-uuid repeat-id group-var-uuid] path
                            input (get first-row k)
                            ws-uuid @wip-ws-uuid]
                        #_[(:value input) (:units input)]
                        (rf/dispatch [:worksheet/add-input-group
                                      ws-uuid
                                      group-uuid
                                      repeat-id])
                        (rf/dispatch [:worksheet/upsert-input-variable
                                      ws-uuid
                                      group-uuid
                                      repeat-id group-var-uuid
                                      (str (:value input))
                                      (:units input)])))
       input-paths)

  (rf/dispatch [:worksheet/solve @wip-ws-uuid])

  (rf/subscribe [:query worksheet/latest])


  (rf/subscribe [:query '[:find ?e :in $ ?uuid :where [?e :worksheet/uuid ?uuid]] [@wip-ws-uuid]])

  (rf/subscribe [:])

  #_(-> test-cell (ffirst) (fs->bp6) (var-path-by-bp6-code-name))

  #_(rf/subscribe [:wizard/*module "contain"])

  #_(let [*module @(rf/subscribe [:wizard/*module "contain"])]
      (filter #(= (:submodule/io %) "input") @(rf/subscribe [:wizard/submodules *module]))

      (rf/subscribe [:vms/pull '][* {:module/submodules
                                    [* {:submodule/groups
                                        [* {:group/group-variables [* {:variable/_group-variables [*]}]}]}]}] (:db/id *module)]))

  #_(count (d/q '[:find ?e ?uuid :where [?e :worksheet/uuid ?uuid]] @@conn))

  #_(rf/dispatch [:worksheet/upsert-variable])

  #_(rf/subscribe [:vms/query '[:find ?g-uuid ?gv-uuid
                                :where [?e :variable/bp6-code-name "vContainAttackDist"]
                                [?e :variable/group-variables ?gv]
                                [?g :group/group-variables ?gv]
                                [?gv :bp/uuid ?gv-uuid]
                                [?g :bp/uuid ?g-uuid]]])

  #_(var-path-by-bp6-code-name "vContainAttackDist")

  #_(rf/subscribe [:vms/query '[:find ?e
                                :where [?e :variable/kind "continuous"]
                                :where [?e :variable/bp6-code-name "vContainAttackDist"]]])

  (.reload (.-location js/window))
  (.-appVersion js/navigator)

  #_(rf/subscribe [:vms/pull '[*] 2488])
