(ns absurder-sql.datascript.stress-test
  (:require
   ["datascript-rs" :refer [WasmDataScript]]
   [absurder-sql.datascript.core :as d]
   [absurder-sql.datascript.impl-rust :as impl-rust]
   [absurder-sql.datascript.persistent-sorted-set :as pss]
   [absurder-sql.interface :as sql]
   [cljs.core.async :refer [go]]
   [cljs.core.async.interop :refer-macros [<p!]]
   [cljs.test :refer [async deftest is testing use-fixtures]]))

(defn- with-wasm []
  (async done
         (go
           (<p! (pss/ensure-initialized!))
           (<p! (sql/init!))
           (done))))

(use-fixtures :once {:before with-wasm})

;;; Helpers

(defn- log! [& args]
  (apply js/console.log "[stress]" (map str args)))

(def ^:private batch-size 10000)

(defn- transact-batched!
  "Transact `n` entities in batches via Rust, storing after each batch.
   Returns the WasmDataScript instance."
  [rdb n db-name]
  (let [batches (partition-all batch-size (range 1 (inc n)))]
    (doseq [batch batches]
      (.transact rdb (pr-str (mapv (fn [i] {:db/id (- i) :val (str i)}) batch))))
    (.storeToLegacy rdb db-name)
    rdb))

(defn- run-entity-stress!
  "Transact `n` entities, store via Rust, restore, and verify.
   Returns a Promise."
  [n]
  (let [db-name (str "stress-" (random-uuid) ".db")
        label   (str n)]
    (-> (sql/connect! db-name)
        (.then
         (fn [_sql-conn]
           (let [schema (clj->js {":val" {}})
                 rdb    (.emptyDb WasmDataScript schema)]
             (transact-batched! rdb n db-name)
             (let [restored (.restoreFromLegacy WasmDataScript db-name)
                   cljs-db  (impl-rust/sync-from-rust restored)]
               (is (= n (count (d/datoms cljs-db :eavt)))
                   (str "Expected " n " datoms")))))))))

;;; Entity scale tests

(deftest ^:stress stress-1k-test
  (testing "1,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 1000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-10k-test
  (testing "10,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 10000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-50k-test
  (testing "50,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 50000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-100k-test
  (testing "100,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 100000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-500k-test
  (testing "500,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 500000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-1m-test
  (testing "1,000,000 entities"
    (async done
           (go
             (try
               (<p! (run-entity-stress! 1000000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

;;; Shape tests (wide, large values, cardinality-many, incremental txs)

(deftest ^:stress stress-wide-entities-test
  (testing "100 entities with 50 attributes each"
    (async done
           (go
             (try
               (let [db-name  (str "stress-wide-" (random-uuid) ".db")
                     _        (<p! (sql/connect! db-name))
                     attrs    (mapv #(keyword (str "attr-" %)) (range 50))
                     js-schema (clj->js (into {} (map (fn [a] [(str ":" (name a)) {}]) attrs)))
                     rdb      (.emptyDb WasmDataScript js-schema)
                     tx-data  (vec (for [i (range 1 101)]
                                     (into {:db/id (- i)}
                                           (map (fn [a] [a (str (name a) "-" i)]) attrs))))]
                 (.transact rdb (pr-str tx-data))
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       cljs-db  (impl-rust/sync-from-rust restored)]
                   (is (= 5000 (count (d/datoms cljs-db :eavt)))
                       "100 entities * 50 attrs = 5000 datoms")))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-large-values-test
  (testing "100 entities with 10KB string values"
    (async done
           (go
             (try
               (let [db-name  (str "stress-large-" (random-uuid) ".db")
                     _        (<p! (sql/connect! db-name))
                     rdb      (.emptyDb WasmDataScript (clj->js {":blob" {}}))
                     big-str  (apply str (repeat 10000 "x"))
                     tx-data  (mapv (fn [i] {:db/id (- i) :blob (str i "-" big-str)})
                                    (range 1 101))]
                 (.transact rdb (pr-str tx-data))
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       cljs-db  (impl-rust/sync-from-rust restored)]
                   (is (= 100 (count (d/datoms cljs-db :eavt))))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-cardinality-many-test
  (testing "10 entities with 500 cardinality-many values each"
    (async done
           (go
             (try
               (let [db-name  (str "stress-cm-" (random-uuid) ".db")
                     _        (<p! (sql/connect! db-name))
                     rdb      (.emptyDb WasmDataScript
                                (clj->js {":tags" {"db/cardinality" "db.cardinality/many"}}))
                     tx-data  (vec (for [i (range 1 11)]
                                     (into {:db/id (- i)}
                                           [[:tags (mapv #(str "tag-" i "-" %) (range 500))]])))]
                 (.transact rdb (pr-str tx-data))
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       cljs-db  (impl-rust/sync-from-rust restored)]
                   (is (= 5000 (count (d/datoms cljs-db :eavt)))
                       "10 entities * 500 tags = 5000 datoms")))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-incremental-transactions-test
  (testing "500 small transactions accumulate correctly"
    (async done
           (go
             (try
               (let [db-name (str "stress-inc-" (random-uuid) ".db")
                     _       (<p! (sql/connect! db-name))
                     rdb     (.emptyDb WasmDataScript (clj->js {":counter" {}}))
                     n       500]
                 (dotimes [i n]
                   (.transact rdb (pr-str [{:db/id -1 :counter (str i)}])))
                 (.storeToLegacy rdb db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       cljs-db  (impl-rust/sync-from-rust restored)]
                   (is (= n (count (d/datoms cljs-db :eavt))))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(defn- run-export-import-stress!
  "Transact `n` entities, store, export, import into fresh DB, restore, verify."
  [n]
  (let [db-name (str "stress-exp-" (random-uuid) ".db")
        label   (str n)]
    (-> (sql/connect! db-name)
        (.then
         (fn [sql-conn]
           (let [schema (clj->js {":val" {}})
                 rdb    (.emptyDb WasmDataScript schema)]
             (transact-batched! rdb n db-name)
             (-> (sql/export! sql-conn)
                 (.then
                  (fn [db-bytes]
                    (log! label "export:" (.-length db-bytes) "bytes")
                    (sql/close! sql-conn)
                    db-bytes))
                 (.then
                  (fn [db-bytes]
                    (let [import-name (str "stress-import-" (random-uuid) ".db")]
                      (-> (sql/connect! import-name)
                          (.then (fn [tmp-conn]
                                   (-> (sql/import! tmp-conn db-bytes)
                                       (.then (fn [_] (sql/close! tmp-conn))))))
                          (.then (fn [_] (sql/connect! import-name)))
                          (.then (fn [_]
                                   (let [restored (.restoreFromLegacy WasmDataScript import-name)
                                         cljs-db  (impl-rust/sync-from-rust restored)]
                                     (is (= n (count (d/datoms cljs-db :eavt)))
                                         (str "Expected " n " datoms after import"))))))))))))))))

(deftest ^:stress stress-export-import-10k-test
  (testing "export/import roundtrip at 10k entities"
    (async done
           (go
             (try
               (<p! (run-export-import-stress! 10000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-export-import-100k-test
  (testing "export/import roundtrip at 100k entities"
    (async done
           (go
             (try
               (<p! (run-export-import-stress! 100000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))

(deftest ^:stress stress-export-import-1m-test
  (testing "export/import roundtrip at 1M entities"
    (async done
           (go
             (try
               (<p! (run-export-import-stress! 1000000))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally (done)))))))
