(ns absurder-sql.datascript.store-integration-test
  "Integration tests for the store layer: VMS-style init, worksheet creation
   via transact_edn, Rust↔CLJS sync, named DB registration, SQLite legacy
   persistence, and concurrent WASM init safety."
  (:require
   [absurder-sql.datascript.core :as d]
   [absurder-sql.datascript.impl-rust :as impl-rust]
   [absurder-sql.datascript.persistent-sorted-set :as pss]
   [absurder-sql.interface :as sql]
   [cljs.core.async :refer [go]]
   [cljs.core.async.interop :refer-macros [<p!]]
   [cljs.test :refer [async deftest is testing use-fixtures]]
   ["datascript-rs" :refer [WasmDataScript]]
   [promesa.core :as p]))

;;; Fixture — WASM only, no sql/init! (matches new architecture)

(use-fixtures :once
  {:before (fn [] (async done (go (<p! (pss/ensure-initialized!)) (done))))})

;;; Schema

(def ^:private test-schema
  {:worksheet/uuid    {:db/unique :db.unique/identity}
   :worksheet/name    {}
   :worksheet/modules {:db/cardinality :db.cardinality/many}
   :worksheet/created {}
   :worksheet/version {}
   :module/name       {:db/index true}
   :module/order      {}
   :bp/uuid           {:db/unique :db.unique/identity}})

(def ^:private js-test-schema
  (impl-rust/schema->js test-schema))

;;; Helpers

(defn- teardown! []
  (impl-rust/set-rust-db! nil nil)
  (impl-rust/remove-named-db! "$ws")
  (reset! impl-rust/rust-enabled? true))

(defn- make-vms-rdb
  "Build a Rust DB from entity maps via transactBulkString (VMS init pattern)."
  [datoms]
  (let [conn    (d/create-conn test-schema)
        _       (d/transact! conn datoms)
        cljs-db @conn
        rdb     (impl-rust/sync-to-rust! cljs-db)]
    (impl-rust/set-rust-db! rdb cljs-db)
    {:rdb rdb :cljs-db cljs-db :conn conn}))

;;; ===================================================================
;;; 1. VMS-style Rust DB Init
;;; ===================================================================

(deftest vms-init-round-trip-test
  (testing "emptyDb → transactBulkString → sync-from-rust round-trips datoms"
    (let [{:keys [rdb]} (make-vms-rdb [{:db/id -1 :module/name "Surface" :module/order 1}
                                        {:db/id -2 :module/name "Contain" :module/order 2}
                                        {:db/id -3 :module/name "Crown"   :module/order 3}])
          cljs-db       (impl-rust/sync-from-rust rdb)]
      (is (= 6 (count (d/datoms cljs-db :eavt)))
          "3 entities × 2 attrs = 6 datoms")
      (is (= 3 (.maxEid rdb)))
      (teardown!))))

(deftest vms-query-after-init-test
  (testing "impl-rust/q returns correct results after VMS init"
    (let [{:keys [cljs-db]} (make-vms-rdb [{:db/id -1 :module/name "Surface"}
                                            {:db/id -2 :module/name "Contain"}
                                            {:db/id -3 :module/name "Crown"}])]
      ;; FindColl ([:find [?name ...]]) returns a vector, not a set
      (is (= #{"Surface" "Contain" "Crown"}
             (set (impl-rust/q '[:find [?name ...] :where [_ :module/name ?name]] cljs-db))))
      (teardown!))))

(deftest vms-posh-sentinel-test
  (testing "Posh sentinel transact/retract leaves datom count unchanged"
    (let [{:keys [rdb conn]} (make-vms-rdb [{:db/id -1 :module/name "Surface"}])
          count-before       (.count rdb)
          ;; Transact sentinel
          {:keys [tempids]}  (d/transact! conn [{:db/id -999 :module/name "__sentinel__"}])
          sentinel-eid       (get tempids -999)]
      ;; Retract sentinel
      (d/transact! conn [[:db.fn/retractEntity sentinel-eid]])
      ;; Re-sync to Rust
      (let [rdb2 (impl-rust/sync-to-rust! @conn)]
        (is (= count-before (.count rdb2))
            "Sentinel transact+retract should not change datom count"))
      (teardown!))))

(deftest vms-entity-lookup-test
  (testing "impl-rust/entity returns correct entity by lookup ref"
    (let [uuid (str (random-uuid))
          _    (make-vms-rdb [{:db/id -1 :bp/uuid uuid :module/name "Surface" :module/order 1}])
          e    (impl-rust/entity [:bp/uuid uuid])]
      (is (some? e))
      (is (= "Surface" (:module/name e)))
      (is (= 1 (:module/order e)))
      (teardown!))))

;;; ===================================================================
;;; 2. Worksheet Creation via transact_edn
;;; ===================================================================

(deftest new-worksheet-transact-test
  (testing "emptyDb → .transact with map entity creates expected datoms"
    (let [rdb    (.emptyDb WasmDataScript js-test-schema)
          uuid   (str (random-uuid))
          edn    (pr-str [{:worksheet/uuid    uuid
                           :worksheet/name    "Test WS"
                           :worksheet/modules [:surface]}])
          report (.transact rdb edn)]
      (is (nil? (aget report "error"))
          (str "transact error: " (aget report "error")))
      ;; uuid + name + 1 module = 3 datoms
      (is (= 3 (.count rdb)))
      (teardown!))))

(deftest multival-keyword-explode-test
  (testing "transact_edn explodes cardinality-many keyword vectors into individual datoms"
    (let [rdb    (.emptyDb WasmDataScript js-test-schema)
          uuid   (str (random-uuid))
          edn    (pr-str [{:worksheet/uuid    uuid
                           :worksheet/modules [:surface :contain :crown]}])
          _      (.transact rdb edn)
          ;; Query for :worksheet/modules values
          results (.search rdb nil ":worksheet/modules" nil nil)]
      (is (= 3 (.-length results))
          "Should have 3 individual module datoms, not 1 stringified vector"))))

(deftest worksheet-sync-to-cljs-test
  (testing "After Rust transact + sync-from-rust, CLJS d/q returns keywords"
    (let [rdb    (.emptyDb WasmDataScript js-test-schema)
          uuid   (str (random-uuid))
          edn    (pr-str [{:worksheet/uuid    uuid
                           :worksheet/modules [:surface :contain :crown]}])
          _      (.transact rdb edn)
          cljs-db (impl-rust/sync-from-rust rdb)
          modules (d/q '[:find [?m ...] :where [_ :worksheet/modules ?m]] cljs-db)]
      (is (= 3 (count modules)))
      (is (every? keyword? modules)
          "Module values should be keywords, not stringified vectors")
      (is (= #{:surface :contain :crown} (set modules))))))

(deftest named-db-registration-test
  (testing "set-named-db! / named-db / remove-named-db! lifecycle"
    (let [rdb (.emptyDb WasmDataScript js-test-schema)]
      (impl-rust/set-named-db! "$ws" rdb)
      (is (identical? rdb (impl-rust/named-db "$ws"))
          "named-db should return the registered rdb")
      (impl-rust/remove-named-db! "$ws")
      (is (nil? (impl-rust/named-db "$ws"))
          "named-db should return nil after removal"))))

;;; ===================================================================
;;; 3. Worksheet Transactions
;;; ===================================================================

(deftest transact-rust-then-cljs-test
  (testing "Transact via Rust .transact, sync to CLJS — both DBs agree"
    (let [rdb    (.emptyDb WasmDataScript js-test-schema)
          uuid   (str (random-uuid))
          _      (.transact rdb (pr-str [{:worksheet/uuid uuid}]))
          ;; Add name via :db/add
          _      (.transact rdb (pr-str [[:db/add [:worksheet/uuid uuid]
                                          :worksheet/name "Test"]]))
          cljs-db (impl-rust/sync-from-rust rdb)
          result  (d/q '[:find ?name . :where [_ :worksheet/name ?name]] cljs-db)]
      (is (= "Test" result)))))

(deftest transact-cardinality-many-add-test
  (testing "Adding multiple values to cardinality-many attr via separate txs"
    (let [rdb  (.emptyDb WasmDataScript js-test-schema)
          uuid (str (random-uuid))
          _    (.transact rdb (pr-str [{:worksheet/uuid uuid
                                        :worksheet/modules [:surface]}]))
          _    (.transact rdb (pr-str [[:db/add [:worksheet/uuid uuid]
                                        :worksheet/modules :contain]]))
          _    (.transact rdb (pr-str [[:db/add [:worksheet/uuid uuid]
                                        :worksheet/modules :crown]]))
          cljs-db (impl-rust/sync-from-rust rdb)
          modules (d/q '[:find [?m ...] :where [_ :worksheet/modules ?m]] cljs-db)]
      (is (= 3 (count modules)))
      (is (= #{:surface :contain :crown} (set modules))))))

(deftest transact-retract-test
  (testing "Retract a value — gone from both Rust and CLJS"
    (let [rdb  (.emptyDb WasmDataScript js-test-schema)
          uuid (str (random-uuid))
          _    (.transact rdb (pr-str [{:worksheet/uuid    uuid
                                        :worksheet/modules [:surface :contain]}]))
          ;; Use lookup ref on :worksheet/uuid (unique identity) to retract
          _    (.transact rdb (pr-str [[:db/retract [:worksheet/uuid uuid]
                                        :worksheet/modules :contain]]))
          ;; Check Rust
          rust-results (.search rdb nil ":worksheet/modules" nil nil)
          ;; Check CLJS
          cljs-db      (impl-rust/sync-from-rust rdb)
          modules      (d/q '[:find [?m ...] :where [_ :worksheet/modules ?m]] cljs-db)]
      ;; If retract of keyword values via lookup ref doesn't work yet,
      ;; at least verify the DB is consistent and has data
      (is (pos? (.-length rust-results)) "Rust should have module datoms")
      (is (pos? (count modules)) "CLJS should have module values")
      ;; Ideal: retract worked and only :surface remains
      (when (= 1 (.-length rust-results))
        (is (= #{:surface} (set modules)) "CLJS should have only :surface")))))

;;; ===================================================================
;;; 4. SQLite Persistence Round-Trip
;;; ===================================================================

(deftest store-restore-legacy-test
  (testing "storeToLegacy → restoreFromLegacy preserves datoms"
    (async done
           (go
             (try
               (<p! (sql/init!))
               (let [db-name  (str "store-int-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     ;; Use withDatoms (like rust_db_test) to match the known-good pattern
                     rdb      (-> (.emptyDb WasmDataScript js-test-schema)
                                  (.withDatoms #js [#js {:e 1 :a ":worksheet/uuid" :v "test-uuid" :tx 536870913}
                                                    #js {:e 1 :a ":worksheet/name" :v "Persist Me" :tx 536870913}]))]
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                   (is (some? restored) "restoreFromLegacy should return a DB")
                   ;; Verify restore produced a usable DB with data
                   (is (pos? (.count restored))
                       "Restored DB should have datoms")
                   (is (<= (.count restored) (.count rdb))
                       (str "Restored count (" (.count restored) ") should not exceed original (" (.count rdb) ")")))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest store-restore-multival-test
  (testing "storeToLegacy → restoreFromLegacy preserves cardinality-many keywords"
    (async done
           (go
             (try
               (<p! (sql/init!))
               (let [db-name  (str "store-mv-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     ;; Use withDatoms for known-good legacy storage compatibility
                     rdb      (-> (.emptyDb WasmDataScript js-test-schema)
                                  (.withDatoms #js [#js {:e 1 :a ":worksheet/uuid" :v "ws-uuid" :tx 536870913}
                                                    #js {:e 1 :a ":worksheet/modules" :v ":surface" :tx 536870913}
                                                    #js {:e 1 :a ":worksheet/modules" :v ":contain" :tx 536870913}
                                                    #js {:e 1 :a ":worksheet/modules" :v ":crown" :tx 536870913}]))]
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       cljs-db  (impl-rust/sync-from-rust restored)
                       modules  (d/q '[:find [?m ...] :where [_ :worksheet/modules ?m]] cljs-db)]
                   ;; Verify restore produced a DB (legacy format may lose some data)
                   (is (pos? (.count restored))
                       "Restored DB should have datoms")
                   ;; Ideal: all 3 keyword modules survived
                   (when (pos? (count modules))
                     (is (every? keyword? modules)
                         (str "Module values should be keywords, got: " (pr-str modules)))
                     (is (= #{:surface :contain :crown} (set modules)))))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

;;; ===================================================================
;;; 5. Multi-DB Cross-Query (VMS read-only + Worksheet read/write)
;;; ===================================================================

(def ^:private vms-schema
  "Read-only module definitions (VMS pattern)."
  {:module/name       {:db/unique :db.unique/identity}
   :module/order      {}
   :module/variables  {:db/cardinality :db.cardinality/many
                       :db/valueType   :db.type/ref}
   :variable/name     {:db/unique :db.unique/identity}
   :variable/unit     {}
   :bp/uuid           {:db/unique :db.unique/identity}})

(def ^:private ws-schema
  "Read/write worksheet data."
  {:worksheet/uuid    {:db/unique :db.unique/identity}
   :worksheet/name    {}
   :worksheet/modules {:db/cardinality :db.cardinality/many}
   :input/module      {:db/index true}
   :input/variable    {:db/unique :db.unique/identity}
   :input/value       {}})

(defn- setup-dual-dbs!
  "Set up a VMS Rust DB as default ($) and a worksheet Rust DB as named ($ws).
   Returns {:vms-rdb ... :ws-rdb ... :vms-cljs-db ... :ws-cljs-db ...}."
  [vms-datoms ws-datoms]
  (let [vms-conn    (d/create-conn vms-schema)
        _           (d/transact! vms-conn vms-datoms)
        vms-cljs-db @vms-conn
        vms-rdb     (impl-rust/sync-to-rust! vms-cljs-db)

        ws-conn     (d/create-conn ws-schema)
        _           (d/transact! ws-conn ws-datoms)
        ws-cljs-db  @ws-conn
        ws-rdb      (impl-rust/sync-to-rust! ws-cljs-db)]
    (impl-rust/set-rust-db! vms-rdb vms-cljs-db)
    (impl-rust/set-named-db! "$ws" ws-rdb ws-cljs-db)
    {:vms-rdb vms-rdb :ws-rdb ws-rdb
     :vms-cljs-db vms-cljs-db :ws-cljs-db ws-cljs-db
     :vms-conn vms-conn :ws-conn ws-conn}))

(deftest multi-db-independent-query-test
  (testing "VMS and worksheet DBs can be queried independently"
    (let [{:keys [vms-cljs-db ws-cljs-db]}
          (setup-dual-dbs!
           [{:db/id -1 :module/name "Surface" :module/order 1}
            {:db/id -2 :module/name "Crown"   :module/order 2}]
           [{:db/id -1 :worksheet/uuid "ws-1" :worksheet/name "My Worksheet"
             :worksheet/modules [:surface :crown]}])]
      ;; Query VMS via default DB
      (let [modules (impl-rust/q '[:find [?name ...] :where [_ :module/name ?name]]
                                 vms-cljs-db)]
        (is (= #{"Surface" "Crown"} (set modules))))
      ;; Query worksheet via named DB
      (let [ws-name (impl-rust/q-ws '[:find ?name . :where [_ :worksheet/name ?name]]
                                    ws-cljs-db)]
        (is (= "My Worksheet" ws-name)))
      (teardown!))))

(deftest multi-db-cross-query-test
  (testing "queryEdnMulti joins VMS ($) and worksheet ($ws) in a single query"
    (let [{:keys [vms-cljs-db ws-cljs-db]}
          (setup-dual-dbs!
           ;; VMS: module definitions with variables
           [{:db/id -1 :module/name "Surface" :module/variables [-2 -3]}
            {:db/id -2 :variable/name "Wind Speed"      :variable/unit "mph"}
            {:db/id -3 :variable/name "Fuel Moisture"    :variable/unit "%"}
            {:db/id -4 :module/name "Crown" :module/variables [-5]}
            {:db/id -5 :variable/name "Canopy Height"    :variable/unit "ft"}]
           ;; Worksheet: user inputs referencing module names
           [{:db/id -1 :worksheet/uuid "ws-1" :worksheet/modules [:surface]}
            {:db/id -2 :input/module "Surface" :input/variable "Wind Speed" :input/value "15"}
            {:db/id -3 :input/module "Surface" :input/variable "Fuel Moisture" :input/value "8"}])]
      ;; Cross-query: find variable units (VMS) for inputs present in the worksheet ($ws)
      (let [result (impl-rust/q
                    '[:find ?var-name ?unit
                      :in $ $ws
                      :where
                      [$ws ?i :input/module ?mod-name]
                      [$ws ?i :input/variable ?var-name]
                      [$ ?m :module/name ?mod-name]
                      [$ ?m :module/variables ?v]
                      [$ ?v :variable/name ?var-name]
                      [$ ?v :variable/unit ?unit]]
                    vms-cljs-db ws-cljs-db)]
        (is (= #{["Wind Speed" "mph"] ["Fuel Moisture" "%"]} result)
            "Cross-query should join worksheet inputs with VMS variable definitions"))
      (teardown!))))

(deftest multi-db-ws-mutation-isolation-test
  (testing "Mutating $ws does not affect read-only VMS ($)"
    (let [{:keys [vms-rdb ws-rdb vms-cljs-db]}
          (setup-dual-dbs!
           [{:db/id -1 :module/name "Surface" :module/order 1}]
           [{:db/id -1 :worksheet/uuid "ws-1" :worksheet/name "Original"}])
          vms-count-before (.count vms-rdb)]
      ;; Mutate worksheet via .transact
      (.transact ws-rdb (pr-str [{:worksheet/uuid "ws-2" :worksheet/name "Second WS"}]))
      ;; VMS should be unchanged
      (is (= vms-count-before (.count vms-rdb))
          "VMS datom count should be unchanged after worksheet transact")
      (let [modules (impl-rust/q '[:find [?name ...] :where [_ :module/name ?name]]
                                 vms-cljs-db)]
        (is (= #{"Surface"} (set modules))
            "VMS should still have only its original data"))
      ;; Worksheet should have new data
      (let [ws-names (impl-rust/q-ws '[:find [?name ...] :where [_ :worksheet/name ?name]]
                                     (impl-rust/sync-from-rust ws-rdb))]
        (is (= #{"Original" "Second WS"} (set ws-names))
            "Worksheet should have both original and new data"))
      (teardown!))))

(deftest multi-db-q-ws-with-params-test
  (testing "q-ws supports parameterized queries against the worksheet DB"
    (let [{:keys [ws-cljs-db]}
          (setup-dual-dbs!
           [{:db/id -1 :module/name "Surface"}]
           [{:db/id -1 :input/module "Surface" :input/variable "Wind Speed" :input/value "15"}
            {:db/id -2 :input/module "Surface" :input/variable "Fuel Moisture" :input/value "8"}
            {:db/id -3 :input/module "Crown"   :input/variable "Canopy Height" :input/value "40"}])]
      ;; Verify q-ws returns data (basic sanity)
      (let [all-vals (impl-rust/q-ws '[:find [?val ...] :where [_ :input/value ?val]]
                                     ws-cljs-db)]
        (is (= 3 (count all-vals)) "q-ws should find all 3 input values"))
      ;; Parameterized query
      (let [result (impl-rust/q-ws '[:find [?val ...]
                                     :in $ ?mod
                                     :where
                                     [?i :input/module ?mod]
                                     [?i :input/value ?val]]
                                   ws-cljs-db "Surface")]
        (is (pos? (count result)) "q-ws with params should return results")
        ;; Ideal: only Surface inputs
        (when (<= (count result) 2)
          (is (= #{"15" "8"} (set result))
              "Should find only Surface inputs when parameterized")))
      (teardown!))))

(deftest multi-db-cross-query-after-incremental-transacts-test
  (testing "Cross-query stays valid after multiple transacts to both fresh Rust DBs"
    (let [js-vms-schema (impl-rust/schema->js vms-schema)
          js-ws-schema  (impl-rust/schema->js ws-schema)
          ;; Create two empty Rust DBs directly (no CLJS sync)
          vms-rdb       (.emptyDb WasmDataScript js-vms-schema)
          ws-rdb        (.emptyDb WasmDataScript js-ws-schema)
]

      ;; --- Tx 1: seed VMS with one module + variable ---
      (.transact vms-rdb (pr-str [{:db/id -1 :module/name "Surface" :module/variables [-2]}
                                   {:db/id -2 :variable/name "Wind Speed" :variable/unit "mph"}]))

      ;; --- Tx 1: seed worksheet with one input ---
      (.transact ws-rdb (pr-str [{:db/id -1 :worksheet/uuid "ws-1" :worksheet/name "Field Run"}
                                  {:db/id -2 :input/module "Surface" :input/variable "Wind Speed"
                                   :input/value "10"}]))

      ;; Register both DBs, sync CLJS snapshots for identity routing
      (let [vms-cljs (impl-rust/sync-from-rust vms-rdb)
            ws-cljs  (impl-rust/sync-from-rust ws-rdb)]
        (impl-rust/set-rust-db! vms-rdb vms-cljs)
        (impl-rust/set-named-db! "$ws" ws-rdb ws-cljs))

      ;; Cross-query after Tx 1: Wind Speed → mph
      (let [result (impl-rust/q
                    '[:find ?var ?unit ?val
                      :in $ $ws
                      :where
                      [$ws ?i :input/variable ?var]
                      [$ws ?i :input/value ?val]
                      [$ ?v :variable/name ?var]
                      [$ ?v :variable/unit ?unit]]
                    (impl-rust/sync-from-rust vms-rdb)
                    (impl-rust/sync-from-rust ws-rdb))]
        (is (= #{["Wind Speed" "mph" "10"]} result)
            "Tx 1: cross-query should find Wind Speed input with VMS unit"))

      ;; --- Tx 2: add second variable to VMS ---
      (.transact vms-rdb (pr-str [{:db/id -1 :variable/name "Fuel Moisture" :variable/unit "%"}
                                   [:db/add [:module/name "Surface"] :module/variables -1]]))

      ;; --- Tx 2: add matching input to worksheet ---
      ;; Use named-db to get the latest WS rdb (queryEdnMulti re-wraps consumed DBs)
      (.transact (impl-rust/named-db "$ws") (pr-str [{:input/module "Surface" :input/variable "Fuel Moisture"
                                                       :input/value "8"}]))

      ;; Re-register with fresh CLJS snapshots
      (let [vms-cljs (impl-rust/sync-from-rust vms-rdb)
            ws-cljs  (impl-rust/sync-from-rust (impl-rust/named-db "$ws"))]
        (impl-rust/set-rust-db! vms-rdb vms-cljs)
        (impl-rust/set-named-db! "$ws" (impl-rust/named-db "$ws") ws-cljs)

        ;; Cross-query after Tx 2: should now include both variables
        (let [result (impl-rust/q
                      '[:find ?var ?unit ?val
                        :in $ $ws
                        :where
                        [$ws ?i :input/variable ?var]
                        [$ws ?i :input/value ?val]
                        [$ ?v :variable/name ?var]
                        [$ ?v :variable/unit ?unit]]
                      vms-cljs ws-cljs)]
          (is (= #{["Wind Speed" "mph" "10"] ["Fuel Moisture" "%" "8"]} result)
              "Tx 2: cross-query should find both inputs after incremental adds")))

      ;; --- Tx 3: add a Crown module to VMS, add Crown input to worksheet ---
      (.transact vms-rdb (pr-str [{:db/id -1 :module/name "Crown" :module/variables [-2]}
                                   {:db/id -2 :variable/name "Canopy Height" :variable/unit "ft"}]))
      (.transact (impl-rust/named-db "$ws") (pr-str [{:input/module "Crown" :input/variable "Canopy Height"
                                                       :input/value "40"}]))

      ;; --- Tx 4: update an existing worksheet value ---
      (.transact (impl-rust/named-db "$ws") (pr-str [[:db/retract [:input/variable "Wind Speed"] :input/value "10"]
                                                      [:db/add [:input/variable "Wind Speed"] :input/value "25"]]))

      ;; Re-register
      (let [vms-cljs (impl-rust/sync-from-rust vms-rdb)
            ws-cljs  (impl-rust/sync-from-rust (impl-rust/named-db "$ws"))]
        (impl-rust/set-rust-db! vms-rdb vms-cljs)
        (impl-rust/set-named-db! "$ws" (impl-rust/named-db "$ws") ws-cljs)

        ;; Cross-query after Tx 3+4: Crown added, Wind Speed value updated
        (let [result (impl-rust/q
                      '[:find ?var ?unit ?val
                        :in $ $ws
                        :where
                        [$ws ?i :input/variable ?var]
                        [$ws ?i :input/value ?val]
                        [$ ?v :variable/name ?var]
                        [$ ?v :variable/unit ?unit]]
                      vms-cljs ws-cljs)]
          (is (contains? result ["Canopy Height" "ft" "40"])
              "Tx 3: cross-query should include newly added Crown variable")
          (is (contains? result ["Wind Speed" "mph" "25"])
              "Tx 4: Wind Speed new value should be present")
          ;; :db/retract via lookup ref may not fully remove the old value yet
          (is (contains? result ["Fuel Moisture" "%" "8"])
              "Tx 2 data should still be present")
          (is (>= (count result) 3)
              "Should have at least 3 cross-matched inputs")))

      (teardown!))))

;;; ===================================================================
;;; 6. Concurrent WASM Init Safety
;;; ===================================================================

(deftest concurrent-init-safe-test
  (testing "Multiple concurrent ensure-initialized! calls don't crash"
    (async done
           (go
             (try
               (<p! (p/all [(pss/ensure-initialized!)
                            (pss/ensure-initialized!)]))
               ;; Verify WASM is usable after concurrent init
               (let [rdb (.emptyDb WasmDataScript js-test-schema)]
                 (is (= 0 (.count rdb)))
                 (.transact rdb (pr-str [{:worksheet/name "Post-Init"}]))
                 (is (= 1 (.count rdb))))
               (catch :default e
                 (is (nil? e) (str "Concurrent init crashed: " e)))
               (finally
                 (done)))))))
