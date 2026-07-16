(ns absurder-sql.datascript.rust-db-test
  "Tests for WasmDataScript — Rust-backed DataScript database.
   Exercises the Rust DB directly via WASM exports, including
   the legacy EDN storage format (restoreFromLegacy/storeToLegacy)."
  (:require
   ["datascript-rs" :refer [WasmDataScript]]
   [absurder-sql.datascript.core :as d]
   [absurder-sql.datascript.impl-rust :as impl-rust]
   [absurder-sql.datascript.persistent-sorted-set :as pss]
   [absurder-sql.interface :as sql]
   [cljs.core.async :refer [go]]
   [cljs.core.async.interop :refer-macros [<p!]]
   [cljs.test :refer [async deftest is testing use-fixtures]]))

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
  (js-schema {":name"   {":db/index" true}
              ":age"    {}
              ":email"  {":db/unique" ":db.unique/identity"}
              ":parent" {":db/valueType" ":db.type/ref"}
              ":aka"    {":db/cardinality" ":db.cardinality/many"}}))

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
    (let [db  (-> (.emptyDb WasmDataScript (test-schema))
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
    (let [db  (-> (.emptyDb WasmDataScript (test-schema))
                  (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                    #js {:e 1 :a ":age" :v 30 :tx 536870913}]))
          all (.datomsIndex db "aevt" nil nil nil nil nil nil nil nil)]
      (is (= 2 (.-length all)))
      ;; AEVT order: :age before :name (alphabetical)
      (let [first-a (.-a (aget all 0))]
        (is (or (= ":age" first-a) (= "age" first-a)))))))

(deftest datoms-avet-indexed-test
  (testing "datomsIndex returns indexed attrs in AVET"
    (let [db  (-> (.emptyDb WasmDataScript (test-schema))
                  (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                    #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                    #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))
          ;; AVET should have :name (indexed) but not :age (not indexed)
          all (.datomsIndex db "avet" nil nil nil nil nil nil nil nil)]
      ;; Only :name datoms (2) — :age is not indexed
      (is (= 2 (.-length all))))))

(deftest with-datom-indexing-test
  (testing "Indexed attrs go to AVET, non-indexed skip AVET"
    (let [db   (-> (.emptyDb WasmDataScript (test-schema))
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
    (let [db  (-> (.emptyDb WasmDataScript (test-schema))
                  (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                    #js {:e 1 :a ":age" :v 30 :tx 536870913}]))
          _   (is (= 2 (.count db)))
          ;; Retract :name (negative tx)
          db2 (.withDatoms db #js [#js {:e 1 :a ":name" :v "Alice" :tx -536870913}])]
      (is (= 1 (.count db2)))
      ;; AVET should also lose the :name datom
      (let [avet (.datomsIndex db2 "avet" nil nil nil nil nil nil nil nil)]
        (is (= 0 (.-length avet)))))))

(deftest cardinality-many-test
  (testing "Multiple values per entity-attr with :db.cardinality/many"
    (let [db      (-> (.emptyDb WasmDataScript (test-schema))
                      (.withDatoms #js [#js {:e 1 :a ":aka" :v "Al" :tx 536870913}
                                        #js {:e 1 :a ":aka" :v "Ali" :tx 536870913}
                                        #js {:e 1 :a ":aka" :v "Alice" :tx 536870913}]))
          results (.search db 1 ":aka" nil nil)]
      (is (= 3 (.-length results))))))

(deftest ref-value-test
  (testing "Ref attrs are indexed in AVET"
    (let [db      (-> (.emptyDb WasmDataScript (test-schema))
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
               (let [db-name  (str "rust-db-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
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
               (let [db-name  (str "legacy-store-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
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
               (let [db-name  (str "legacy-rt-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
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

(deftest legacy-re-store-after-restore-test
  (testing "re-storing after a restore persists the edit without corrupting the db"
    (async done
           (go
             (try
               (let [db-name (str "legacy-restore-" (random-uuid) ".db")]
                 ;; Session 1: create via transact + store + sync (mirrors new-worksheet!)
                 (let [c1  (<p! (sql/connect! db-name))
                       db1 (.emptyDb WasmDataScript (test-schema))]
                   (.transact db1 "[[:db/add 1 :name \"Alice\"]]")
                   (.storeToLegacy db1 db-name)
                   (<p! (sql/sync! c1))
                   (<p! (sql/close! c1)))
                 ;; Session 2: reconnect + restore, then edit-then-re-store on the
                 ;; SAME open connection (mirrors load-store-local! + schedule-persist!)
                 (let [c2  (<p! (sql/connect! db-name))
                       db2 (.restoreFromLegacy WasmDataScript db-name)]
                   (is (some? db2) "first restore returns a DB")
                   (is (= 1 (.count db2)) "restored db has Alice")
                   ;; Edit in place via transact, then re-store (schedule-persist!)
                   (.transact db2 "[[:db/add 2 :name \"Bob\"]]")
                   ;; Currently panics: SQLITE_CORRUPT in write_metadata on re-store.
                   (.storeToLegacy db2 db-name)
                   (<p! (sql/sync! c2))
                   (<p! (sql/close! c2)))
                 ;; Session 3: reconnect + restore + verify BOTH datoms persisted
                 (let [c3  (<p! (sql/connect! db-name))
                       db4 (.restoreFromLegacy WasmDataScript db-name)]
                   (is (some? db4) "second restore returns a DB")
                   (is (= 2 (.count db4))
                       "both Alice and Bob persist after re-store")
                   (is (= 1 (.-length (.search db4 nil ":name" "Bob" nil)))
                       "Bob is found after re-store")
                   (<p! (sql/close! c3))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

;; ===================================================================
;; Self-sufficient persistence lifecycle (Phase A)
;;
;; These tests use ONLY the WasmDataScript lifecycle API — no sql/connect!
;; or sql/sync! — proving the connect-first contract is gone.
;; ===================================================================

(deftest self-sufficient-lifecycle-test
  (testing "open → transact → persist → closeDb → open restores, no sql/* calls"
    (async done
           (go
             (try
               (let [db-name (str "lifecycle-" (random-uuid) ".db")
                     ;; Session 1: open (fresh), transact, persist, close.
                     rdb     (<p! (.open WasmDataScript db-name (test-schema)))]
                 (is (some? rdb) "open returns a fresh db")
                 (is (= 0 (.count rdb)) "fresh db is empty")
                 (.transact rdb "[[:db/add 1 :name \"Alice\"]]")
                 (<p! (.persist rdb db-name))
                 (<p! (.closeDb WasmDataScript db-name))
                 ;; Session 2: reopen restores; edit + re-persist (re-store path).
                 (let [rdb2 (<p! (.open WasmDataScript db-name (test-schema)))]
                   (is (= 1 (.count rdb2)) "reopened db has Alice")
                   (is (= 1 (.-length (.search rdb2 nil ":name" "Alice" nil)))
                       "Alice found after reopen")
                   (.transact rdb2 "[[:db/add 2 :name \"Bob\"]]")
                   (<p! (.persist rdb2 db-name))
                   (<p! (.closeDb WasmDataScript db-name)))
                 ;; Session 3: the edit survived the close/open cycle.
                 (let [rdb3 (<p! (.open WasmDataScript db-name (test-schema)))]
                   (is (= 2 (.count rdb3)) "edit persisted across close/open")
                   (is (= 1 (.-length (.search rdb3 nil ":name" "Bob" nil)))
                       "Bob found after re-store")
                   (<p! (.closeDb WasmDataScript db-name))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest multiple-new-map-entities-distinct-test
  (testing "transacting several :db/id-less map entities creates distinct entities"
    ;; Regression: the synthetic auto-tempid was the same for every bare map
    ;; entity in a tx, so multiple new entities merged into one (e.g. an
    ;; export round-trip losing all but the last worksheet).
    (let [db     (.emptyDb WasmDataScript (test-schema))
          report (.transact db (pr-str [{:name "Alice"}
                                        {:name "Bob"}
                                        {:name "Carol"}]))]
      (is (nil? (aget report "error")))
      (is (= 3 (.count db)) "three distinct entities, not merged into one")
      (let [names (into #{}
                        (map #(.-v %))
                        (array-seq (.datomsIndex db "aevt" nil ":name"
                                                 nil nil nil nil nil nil)))]
        (is (= #{"Alice" "Bob" "Carol"} names) "all three names present"))))
  (testing "upsert still applies: a unique-identity match reuses the entity"
    (let [db (.emptyDb WasmDataScript (test-schema))]
      (.transact db (pr-str [{:email "a@b" :name "Alice"}]))
      ;; Second tx: one new entity + one upsert on the existing :email.
      (.transact db (pr-str [{:email "a@b" :name "Alice2"}
                             {:email "c@d" :name "Carol"}]))
      (is (= 2 (.-length (.search db nil ":email" nil nil)))
          "two distinct entities by unique :email (upsert, not duplicate)")
      (is (= 1 (.-length (.search db nil ":name" "Alice2" nil)))
          "existing entity was updated via upsert"))))

(deftest reverse-ref-lookup-ref-test
  (testing "map tx with a reverse ref whose value is a lookup ref (upsert-output shape)"
    ;; {:worksheet/_outputs [:worksheet/uuid ws-uuid]} — the referring entity
    ;; is identified by lookup ref. Regression: the Rust EDN parser used to
    ;; stringify the lookup-ref vector (reverse attrs aren't in the schema),
    ;; then fail with "Reverse ref value must be an entity ref".
    (let [db     (.emptyDb WasmDataScript (test-schema))
          _      (.transact db "[[:db/add 1 :email \"a@b\"] [:db/add 1 :name \"Alice\"]]")
          report (.transact db (pr-str [{:_parent [:email "a@b"]
                                         :name    "Child"}]))]
      (is (nil? (aget report "error"))
          (str "reverse-ref lookup-ref tx should parse: " (aget report "error")))
      ;; Alice's :parent now points at the new child entity.
      (let [parent-datoms (.search db 1 ":parent" nil nil)]
        (is (= 1 (.-length parent-datoms)) "Alice gained a :parent ref")
        (let [child-eid (.-v (aget parent-datoms 0))]
          (is (= 1 (.-length (.search db child-eid ":name" "Child" nil)))
              "ref points at the Child entity"))))))

(deftest persist-requires-open-test
  (testing "persist without open fails loudly instead of writing to memory"
    (let [db (.emptyDb WasmDataScript (test-schema))]
      (is (thrown? js/Error (.persist db (str "not-open-" (random-uuid) ".db")))
          "persist on an unopened db throws"))))

(deftest store-to-legacy-requires-connection-test
  (testing "storeToLegacy without a connection fails loudly (no in-memory fallback)"
    (let [db (.emptyDb WasmDataScript (test-schema))]
      (is (thrown? js/Error (.storeToLegacy db (str "no-conn-" (random-uuid) ".db")))
          "storeToLegacy on an unopened db throws"))))

(deftest export-import-round-trip-test
  (testing "exportDb bytes can be imported into a new db and restored"
    (async done
           (go
             (try
               (let [db-name (str "export-" (random-uuid) ".db")
                     rdb     (<p! (.open WasmDataScript db-name (test-schema)))]
                 (.transact rdb "[[:db/add 1 :name \"Alice\"] [:db/add 1 :age 30]]")
                 (let [db-bytes (<p! (.exportDb rdb db-name))]
                   (is (pos? (.-length db-bytes)) "export produces bytes")
                   (<p! (.closeDb WasmDataScript db-name))
                   (let [import-name (str "import-" (random-uuid) ".db")]
                     (<p! (.importDb WasmDataScript import-name db-bytes))
                     (let [rdb2 (<p! (.open WasmDataScript import-name (test-schema)))]
                       (is (= 2 (.count rdb2)) "imported db has both datoms")
                       (is (= 1 (.-length (.search rdb2 nil ":name" "Alice" nil)))
                           "Alice found in imported db")
                       (<p! (.closeDb WasmDataScript import-name))))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest mirror-eid-divergence-after-restore-test
  (testing "entities created after a restore get the same eid in Rust and the CLJS mirror"
    ;; Mirrors the behave app's architecture: the Rust rdb is the source of
    ;; truth, a CLJS conn mirrors it for reactivity, and the :transact fx
    ;; replays the SAME tx-data into both engines independently.
    ;;
    ;; Rust persists its max-eid high-water mark in the legacy metadata and
    ;; restores it verbatim; the CLJS mirror (conn-from-datoms) recomputes
    ;; max-eid from the live datoms. After restoring a db that ever retracted
    ;; its top entity, the two allocators diverge, so a new entity gets
    ;; DIFFERENT eids in Rust vs the mirror — and every subsequent edit
    ;; addressed by a mirror eid silently lands on the wrong Rust entity.
    (async done
           (go
             (try
               (let [db-name (str "legacy-eid-div-" (random-uuid) ".db")
                     schema  {:name {:db/index true}
                              :age  {}}]
                 ;; Session 1: two entities, retract the top one, persist.
                 ;; Rust max-eid stays 2 (high water); live max datom eid is 1.
                 (let [c1  (<p! (sql/connect! db-name))
                       db1 (.emptyDb WasmDataScript (test-schema))]
                   (.transact db1 "[[:db/add 1 :name \"Alice\"] [:db/add 2 :name \"Bob\"]]")
                   (is (= 2 (.maxEid db1)) "Bob advanced max-eid to 2")
                   (.transact db1 "[[:db.fn/retractEntity 2]]")
                   (is (zero? (.-length (.search db1 nil ":name" "Bob" nil)))
                       "Bob is retracted before the store")
                   (is (= 2 (.maxEid db1)) "max-eid high water survives the retract")
                   (.storeToLegacy db1 db-name)
                   (<p! (sql/sync! c1))
                   (<p! (sql/close! c1)))
                 ;; Session 2: restore + mirror (as load-store-local! does),
                 ;; then create a new entity through BOTH engines (as the
                 ;; :transact fx does).
                 (let [c2      (<p! (sql/connect! db-name))
                       rdb     (.restoreFromLegacy WasmDataScript db-name)
                       _       (is (= 2 (.maxEid rdb))
                                   "restored Rust db keeps the max-eid high water mark")
                       cljs-db (impl-rust/sync-from-rust rdb)
                       _       (is (= 2 (:max-eid cljs-db))
                                   "sync-from-rust carries the Rust max-eid, not the datom-derived one")
                       conn    (d/conn-from-datoms (d/datoms cljs-db :eavt) schema)
                       ;; conn-from-datoms re-derives max-eid from datoms —
                       ;; align it with the Rust allocator (as behave.store must).
                       _       (swap! conn assoc :max-eid (:max-eid cljs-db))
                       tx      [[:db/add -1 :name "Carol"]]]
                   (.transact rdb (pr-str tx))
                   (d/transact! conn tx)
                   (let [rust-eid   (.-e (aget (.search rdb nil ":name" "Carol" nil) 0))
                         mirror-eid (d/q '[:find ?e . :where [?e :name "Carol"]] @conn)]
                     (is (= rust-eid mirror-eid)
                         "Carol's eid must match between Rust and the CLJS mirror")
                     ;; The app-visible symptom: an edit addressed by the
                     ;; mirror's eid must land on Carol in the Rust db.
                     (.transact rdb (pr-str [[:db/add mirror-eid :age 30]]))
                     (.storeToLegacy rdb db-name)
                     (<p! (sql/sync! c2)))
                   (<p! (sql/close! c2)))
                 ;; Session 3: restore and verify Carol's edit persisted.
                 (let [c3        (<p! (sql/connect! db-name))
                       db3       (.restoreFromLegacy WasmDataScript db-name)
                       carol-eid (.-e (aget (.search db3 nil ":name" "Carol" nil) 0))
                       ages      (.search db3 carol-eid ":age" nil nil)]
                   (is (= 1 (.-length ages))
                       "Carol's :age edit persists across restore")
                   (<p! (sql/close! c3))))
               (catch :default e
                 (is (nil? e) (str "Unexpected error: " e)))
               (finally
                 (done)))))))

(deftest has-legacy-db-test
  (testing "hasLegacyDb returns true after storeToLegacy"
    (async done
           (go
             (try
               (let [db-name  (str "has-legacy-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
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
               (let [db-name  (str "legacy-schema-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
                                  (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                                    #js {:e 1 :a ":email" :v "a@b" :tx 536870913}
                                                    #js {:e 1 :a ":parent" :v 2 :tx 536870913}
                                                    #js {:e 1 :a ":aka" :v "Al" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 (let [restored  (.restoreFromLegacy WasmDataScript db-name)
                       schema-js (.schema restored)
                       entries   (js/Object.entries schema-js)]
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
               (let [db-name  (str "legacy-gc-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db1      (-> (.emptyDb WasmDataScript (js-schema {":val" {}}))
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
               (let [db-name  (str "legacy-export-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     db       (-> (.emptyDb WasmDataScript (test-schema))
                                  (.withDatoms #js [#js {:e 1 :a ":name" :v "Alice" :tx 536870913}
                                                    #js {:e 1 :a ":age" :v 30 :tx 536870913}
                                                    #js {:e 2 :a ":name" :v "Bob" :tx 536870913}]))]
                 (.storeToLegacy db db-name)
                 ;; Export
                 (let [db-bytes (<p! (sql/export! sql-conn))]
                   (<p! (sql/close! sql-conn))
                   ;; Import into fresh connection
                   (let [import-name (str "legacy-import-" (random-uuid) ".db")
                         tmp-conn    (<p! (sql/connect! import-name))]
                     (<p! (sql/import! tmp-conn db-bytes))
                     (<p! (sql/close! tmp-conn))
                     ;; Reconnect and restore
                     (let [fresh-conn (<p! (sql/connect! import-name))
                           restored   (.restoreFromLegacy WasmDataScript import-name)]
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
               (let [db-name  (str "legacy-ns-" (random-uuid) ".db")
                     sql-conn (<p! (sql/connect! db-name))
                     schema   (js-schema {":person/name"   {":db/index" true}
                                          ":person/age"    {}
                                          ":person/parent" {":db/valueType" ":db.type/ref"}})
                     db       (-> (.emptyDb WasmDataScript schema)
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
