(ns absurder-sql.datascript.rust-db-test
  "Tests for WasmDataScript — Rust-backed DataScript database.
   Exercises the Rust DB directly via WASM exports, including
   the legacy EDN storage format (restoreFromLegacy/storeToLegacy)."
  (:require
   [absurder-sql.datascript.persistent-sorted-set :as pss]
   [absurder-sql.interface :as sql]
   [cljs.core.async :refer [go]]
   [cljs.core.async.interop :refer-macros [<p!]]
   [cljs.test :refer [async deftest is testing use-fixtures]]
   ["datascript-rs" :refer [WasmDataScript]]))

(defn- with-sqlite []
  (async done
         (go
           (<p! (pss/ensure-initialized!))
           (<p! (sql/init!))
           (done))))

(use-fixtures :once {:before with-sqlite})

;; Helper: create a JS schema object matching CLJS format
(defn- js-schema [m]
  (clj->js m))

(defn- test-schema []
  (js-schema {":name" {":db/index" true}
              ":age" {}
              ":email" {":db/unique" ":db.unique/identity"}
              ":parent" {":db/valueType" ":db.type/ref"}
              ":aka" {":db/cardinality" ":db.cardinality/many"}}))

;; Helper: create a datom JS object
(defn- js-datom [e a v tx]
  #js {:e e :a a :v v :tx tx})

;; ===================================================================
;; Core WasmDataScript tests
;; ===================================================================

(deftest empty-db-via-rust-test
  (testing "WasmDataScript.emptyDb creates an empty Rust-backed DB"
    (let [db (.emptyDb WasmDataScript (test-schema))]
      (is (= 0 (.count db)))
      (is (= 536870912 (.maxTx db)))
      (is (= 0 (.maxEid db))))))

(deftest transact-and-query-test
  (testing "withDatoms adds datoms, search returns them"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                   #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))]
      (is (= 3 (.count db)))
      (is (= 2 (.maxEid db)))
      ;; Search by entity
      (let [results (.search db 1 nil nil nil)]
        (is (= 2 (.-length results)))))))

(deftest datoms-eavt-test
  (testing "datomsIndex returns sorted datoms for EAVT"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 2 :a ":name" :v "Bob" :tx 536870913}
                                   #js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}]))
          ;; Get all datoms from EAVT
          all (.datomsIndex db "eavt" nil nil nil nil nil nil nil nil)]
      (is (= 3 (.-length all)))
      ;; EAVT order: entity 1 before entity 2
      (is (= 1 (.-e (aget all 0)))))))

(deftest datoms-aevt-test
  (testing "datomsIndex returns attr-sorted datoms for AEVT"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}]))
          all (.datomsIndex db "aevt" nil nil nil nil nil nil nil nil)]
      (is (= 2 (.-length all)))
      ;; AEVT order: :age before :name (alphabetical)
      (let [first-a (.-a (aget all 0))]
        (is (or (= ":age" first-a) (= "age" first-a)))))))

(deftest datoms-avet-indexed-test
  (testing "datomsIndex returns indexed attrs in AVET"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                   #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))
          ;; AVET should have :name (indexed) but not :age (not indexed)
          all (.datomsIndex db "avet" nil nil nil nil nil nil nil nil)]
      ;; Only :name datoms (2) — :age is not indexed
      (is (= 2 (.-length all))))))

(deftest with-datom-indexing-test
  (testing "Indexed attrs go to AVET, non-indexed skip AVET"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                   #js {:e 1 :a ":email" :v "a@b.com" :tx 536870913}
                                   #js {:e 1 :a ":parent" :v 2 :tx 536870913}]))
          avet (.datomsIndex db "avet" nil nil nil nil nil nil nil nil)]
      ;; :name (index), :email (unique→indexed), :parent (ref→indexed) = 3
      ;; :age not indexed
      (is (= 3 (.-length avet))))))

(deftest retraction-test
  (testing "Retracted datoms are removed from all indexes"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                   #js {:e 1 :a ":age" :v 30 :tx 536870913}]))
          _ (is (= 2 (.count db)))
          ;; Retract :name (negative tx)
          db2 (.withDatoms db #js [#js {:e 1 :a ":name" :v "Alice" :tx -536870913}])]
      (is (= 1 (.count db2)))
      ;; AVET should also lose the :name datom
      (let [avet (.datomsIndex db2 "avet" nil nil nil nil nil nil nil nil)]
        (is (= 0 (.-length avet)))))))

(deftest cardinality-many-test
  (testing "Multiple values per entity-attr with :db.cardinality/many"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":aka" :v "Al" :tx 536870913}
                                   #js {:e 1 :a ":aka" :v "Ali" :tx 536870913}
                                   #js {:e 1 :a ":aka" :v "Alice" :tx 536870913}]))
          results (.search db 1 ":aka" nil nil)]
      (is (= 3 (.-length results))))))

(deftest ref-value-test
  (testing "Ref attrs are indexed in AVET"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 1 :a ":parent" :v 2 :tx 536870913}
                                   #js {:e 3 :a ":parent" :v 2 :tx 536870913}]))
          ;; Search by ref value via AVET (refs are implicitly indexed)
          results (.search db nil ":parent" 2 nil)]
      (is (= 2 (.-length results))))))

(deftest max-eid-advancement-test
  (testing "maxEid tracks highest entity id"
    (let [db (-> (.emptyDb WasmDataScript (test-schema))
                 (.withDatoms #js [#js {:e 5 :a ":name" :v "E5" :tx 536870913}
                                   #js {:e 3 :a ":name" :v "E3" :tx 536870913}
                                   #js {:e 10 :a ":name" :v "E10" :tx 536870913}]))]
      (is (= 10 (.maxEid db))))))

;; ===================================================================
;; Binary format store/restore (pss_nodes table)
;; ===================================================================

(deftest store-restore-roundtrip-test
  (testing "Store DB to SQLite, restore, verify datoms match"
    (async done
           (go
             (try
               (let [db-name (str "rust-db-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                              #js {:e 2 :a ":name" :v "Bob" :tx 536870913}
                                              #js {:e 2 :a ":email" :v "b@b.com" :tx 536870913}]))]
                 ;; Store
                 (.storeDb db db-name)
                 ;; Restore
                 (let [restored (.restoreDb WasmDataScript db-name)]
                   (is (some? restored) "restoreDb should return a DB after storeDb")
                   (is (= (.count db) (.count restored)) "count matches after restore")
                   (is (= (.maxEid db) (.maxEid restored)) "max-eid matches")
                   (is (= (.maxTx db) (.maxTx restored)) "max-tx matches")
                   ;; Verify specific search
                   (let [results (.search restored 1 ":name" nil nil)]
                     (is (= 1 (.-length results)))))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

;; ===================================================================
;; Legacy EDN format store/restore (datascript table)
;; ===================================================================

(deftest store-to-legacy-test
  (testing "storeToLegacy writes data to the datascript table in EDN format"
    (async done
           (go
             (try
               (let [db-name (str "legacy-store-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                              #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 ;; Verify the datascript table was created
                 (let [rows (<p! (sql/select sql-conn "SELECT count(*) as cnt FROM datascript"))]
                   (is (pos? (:cnt (first rows)))
                       "datascript table should have rows after storeToLegacy"))
                 ;; Verify metadata at addr=0 is EDN text
                 (let [meta-rows (<p! (sql/select sql-conn "SELECT content FROM datascript WHERE addr = 0"))]
                   (is (some? (:content (first meta-rows)))
                       "metadata at addr 0 should exist")
                   (let [content (:content (first meta-rows))]
                     (is (string? content)
                         "metadata content should be a string (EDN)")
                     (is (.includes content ":schema")
                         "metadata should contain :schema")
                     (is (.includes content ":eavt")
                         "metadata should contain :eavt")))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest restore-from-legacy-test
  (testing "storeToLegacy then restoreFromLegacy round-trips data"
    (async done
           (go
             (try
               (let [db-name (str "legacy-rt-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                              #js {:e 2 :a ":name" :v "Bob" :tx 536870913}
                                              #js {:e 2 :a ":email" :v "b@b.com" :tx 536870913}
                                              #js {:e 1 :a ":parent" :v 2 :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                   (is (some? restored) "restoreFromLegacy should return a DB")
                   (is (= (.count db) (.count restored))
                       (str "count: expected " (.count db) ", got " (.count restored)))
                   (is (= (.maxEid db) (.maxEid restored)) "max-eid matches")
                   (is (= (.maxTx db) (.maxTx restored)) "max-tx matches")
                   ;; Verify search by entity
                   (let [e1-datoms (.search restored 1 nil nil nil)]
                     (is (= 3 (.-length e1-datoms))
                         "entity 1 should have 3 datoms (name, age, parent)"))
                   ;; Verify indexed attr search (AVET)
                   (let [name-search (.search restored nil ":name" "Alice" nil)]
                     (is (= 1 (.-length name-search))
                         "search by indexed :name should find Alice")))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest has-legacy-db-test
  (testing "hasLegacyDb returns true after storeToLegacy"
    (async done
           (go
             (try
               (let [db-name (str "has-legacy-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Test" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 (is (true? (.hasLegacyDb WasmDataScript db-name))
                     "hasLegacyDb should return true after storeToLegacy")
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest legacy-schema-preservation-test
  (testing "Schema properties survive legacy store/restore roundtrip"
    (async done
           (go
             (try
               (let [db-name (str "legacy-schema-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":email" :v "a@b" :tx 536870913}
                                              #js {:e 1 :a ":parent" :v 2 :tx 536870913}
                                              #js {:e 1 :a ":aka" :v "Al" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)
                       schema-js (.schema restored)
                       entries (js/Object.entries schema-js)]
                   ;; Verify schema attrs survived
                   (is (pos? (.-length entries))
                       "restored schema should have attributes")
                   ;; Check specific properties
                   (let [name-props (aget (js/Object.fromEntries entries) ":name")]
                     (is (some? name-props) "schema should have :name")
                     (when name-props
                       (is (true? (aget name-props ":db/index"))
                           ":name should have :db/index true")))
                   (let [email-props (aget (js/Object.fromEntries entries) ":email")]
                     (when email-props
                       (is (= ":db.unique/identity" (aget email-props ":db/unique"))
                           ":email should have :db.unique/identity")))
                   ;; Verify indexing still works after restore
                   (let [avet (.datomsIndex restored "avet" nil nil nil nil nil nil nil nil)]
                     ;; :name (indexed), :email (unique→indexed), :parent (ref→indexed) = 3
                     (is (= 3 (.-length avet))
                         "AVET should have indexed attrs after legacy restore")))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest legacy-gc-test
  (testing "collectGarbage works after storeToLegacy"
    (async done
           (go
             (try
               (let [db-name (str "legacy-gc-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db1 (-> (.emptyDb WasmDataScript (js-schema {":val" {}}))
                             (.withDatoms (let [arr (js/Array.)]
                                           (dotimes [i 50]
                                             (.push arr (js-datom (inc i) ":val" (str i) 536870913)))
                                           arr)))]
                 ;; Store first version
                 (.storeToLegacy db1 db-name)
                 (let [rows-before (<p! (sql/select sql-conn "SELECT count(*) as cnt FROM datascript"))]
                   ;; Add more datoms (creates new tree nodes, old ones become garbage)
                   (let [db2 (.withDatoms db1 (let [arr (js/Array.)]
                                                (dotimes [i 50]
                                                  (.push arr (js-datom (+ 51 i) ":val" (str (+ 51 i)) 536870913)))
                                                arr))]
                     (.storeToLegacy db2 db-name)
                     (let [rows-after (<p! (sql/select sql-conn "SELECT count(*) as cnt FROM datascript"))]
                       ;; GC should clean up old tree nodes
                       (.collectGarbage db2 db-name)
                       (let [rows-gc (<p! (sql/select sql-conn "SELECT count(*) as cnt FROM datascript"))]
                         ;; After GC, there should be fewer or equal rows
                         (is (<= (:cnt (first rows-gc)) (:cnt (first rows-after)))
                             "GC should not increase row count")
                         ;; Verify data is still intact
                         (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                           (is (= (.count db2) (.count restored))
                               "Data should be intact after GC"))))))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest legacy-export-import-roundtrip-test
  (testing "Store to legacy, export SQLite bytes, import into fresh DB, restore"
    (async done
           (go
             (try
               (let [db-name (str "legacy-export-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db (-> (.emptyDb WasmDataScript (test-schema))
                            (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                              #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 ;; Export
                 (let [db-bytes (<p! (sql/export! sql-conn))]
                   (<p! (sql/close! sql-conn))
                   ;; Import into fresh connection
                   (let [import-name (str "legacy-import-" (random-uuid) ".db")
                         tmp-conn (<p! (sql/connect! import-name))]
                     (<p! (sql/import! tmp-conn db-bytes))
                     (<p! (sql/close! tmp-conn))
                     ;; Reconnect and restore
                     (let [fresh-conn (<p! (sql/connect! import-name))
                           restored (.restoreFromLegacy WasmDataScript import-name)]
                       (is (some? restored) "should restore after import")
                       (is (= (.count db) (.count restored))
                           "imported DB should have same datom count")
                       (let [results (.search restored nil ":name" "Alice" nil)]
                         (is (= 1 (.-length results))
                             "should find Alice after import"))
                       (<p! (sql/close! fresh-conn))))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest legacy-namespaced-attrs-test
  (testing "Namespaced attributes survive legacy roundtrip"
    (async done
           (go
             (try
               (let [db-name (str "legacy-ns-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     schema (js-schema {":person/name" {":db/index" true}
                                        ":person/age" {}
                                        ":person/parent" {":db/valueType" ":db.type/ref"}})
                     db (-> (.emptyDb WasmDataScript schema)
                            (.withDatoms #js [#js {:e 1 :a ":person/name" :v "Alice" :tx 536870913}
                                              #js {:e 1 :a ":person/age" :v 30 :tx 536870913}
                                              #js {:e 1 :a ":person/parent" :v 2 :tx 536870913}
                                              #js {:e 2 :a ":person/name" :v "Bob" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                   (is (= (.count db) (.count restored))
                       "count matches")
                   ;; Search by namespaced attr
                   (let [results (.search restored 1 ":person/name" nil nil)]
                     (is (= 1 (.-length results))
                         "should find entity 1's :person/name"))
                   ;; Verify ref attr is in AVET (indexed)
                   (let [avet (.datomsIndex restored "avet" nil nil nil nil nil nil nil nil)]
                     ;; :person/name (indexed) + :person/parent (ref→indexed) = 3 (Alice, Bob, parent)
                     (is (= 3 (.-length avet))
                         "AVET should have indexed namespaced attrs")))
                 (<p! (sql/close! sql-conn)))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))
