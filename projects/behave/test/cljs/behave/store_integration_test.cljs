(ns behave.store-integration-test
  "Integration tests for the full re-frame/re-posh/absurder-sql storage cycle.
   Tests worksheet create, transact, query, save, and restore round-trips."
  (:require ["datascript-rs" :refer [WasmDataScript]]
            [absurder-sql.datascript.core :as d]
            [absurder-sql.datascript.impl-rust :as impl-rust]
            [absurder-sql.datascript.persistent-sorted-set :as pss]
            [absurder-sql.interface :as sql]
            [austinbirch.reactive-entity :as re]
            [behave.schema.core :refer [all-schemas]]
            [behave.store :as s]
            [cljs.core.async :refer [go]]
            [cljs.core.async.interop :refer-macros [<p!]]
            [cljs.test :refer [deftest is async testing use-fixtures] :include-macros true]
            [ds-schema-utils.interface :refer [->ds-schema]]
            [promesa.core :as p]
            [re-frame.core :as rf]
            [re-posh.core :as rp]
            [re-posh.db :as rpdb]))

;;; Fixtures

(defn ^:private setup-rf! []
  ;; Clear Rust-side state at the START of each test too, not only in
  ;; teardown: the async sqlite tests can set "$ws" in a promise continuation
  ;; that runs AFTER their `done`/teardown, so a teardown-only reset misses it
  ;; and the next test's :transact misroutes away from its fresh CLJS conn.
  (impl-rust/clear-all-state!)
  (rf/dispatch-sync [:initialize]))

(defn ^:private teardown! []
  (rf/clear-subscription-cache!)
  (re/clear-cache!)
  ;; Reset re-posh store *after* clearing sub cache to avoid
  ;; stale subscriptions evaluating against nil conn
  (reset! rpdb/store nil)
  (reset! s/conn nil)
  (impl-rust/clear-all-state!))

(use-fixtures :once
  {:before (fn [] (async done (go (<p! (pss/ensure-initialized!)) (done))))})

(use-fixtures :each
  {:before (fn [] (setup-rf!))
   :after  (fn [] (teardown!))})

;;; Helpers

(def ^:private test-schema (->ds-schema all-schemas))

(defn- make-conn!
  "Create an in-memory DataScript conn, wire it into behave.store and re-posh."
  []
  (let [conn (d/create-conn test-schema)]
    (reset! s/conn conn)
    (rp/connect! conn)
    (re/init! conn)
    conn))

;;; ==========================================================================
;;; Multi-Window Safety: persist timer cancellation on worksheet switch
;;; ==========================================================================

(deftest ^:integration load-store-local-cancels-stale-persist-timer-test
  (testing "load-store-local! cancels any pending persist timer from a previous worksheet"
    (async done
           (let [fired?     (atom false)
                 fake-timer (js/setTimeout #(reset! fired? true) 50)]
             (reset! s/persist-timer fake-timer)
             (reset! s/active-db-name "worksheet-stale.db")
             ;; Switching worksheets must cancel the pending timer synchronously
             (s/load-store-local! "fresh-uuid")
             (is (nil? @s/persist-timer)
                 "persist-timer atom should be cleared immediately")
             ;; Wait well past the fake timer's firing window
             (js/setTimeout
              (fn []
                (is (false? @fired?)
                    "stale timer from previous worksheet must not fire after switch")
                (done))
              150)))))

(deftest ^:integration load-store-minimal-cancels-stale-persist-timer-test
  (testing "load-store-minimal! also cancels a pending persist timer"
    (async done
           (let [fired?     (atom false)
                 fake-timer (js/setTimeout #(reset! fired? true) 50)]
             (reset! s/persist-timer fake-timer)
             (reset! s/active-db-name "worksheet-stale.db")
             (s/load-store-minimal!)
             (is (nil? @s/persist-timer)
                 "persist-timer atom should be cleared by load-store-minimal!")
             (js/setTimeout
              (fn []
                (is (false? @fired?)
                    "stale timer must not fire after load-store-minimal!")
                (done))
              150)))))

(deftest ^:integration switch-worksheet-updates-db-name-test
  (testing "after switching worksheets active-db-name reflects the new worksheet"
    (async done
           ;; Prime active-db-name as if a previous worksheet was active
           (reset! s/active-db-name "worksheet-old.db")
           (s/load-store-local! "new-ws-uuid")
           ;; db-name swap happens synchronously inside load-store-local!
           (is (= "worksheet-new-ws-uuid.db" @s/active-db-name)
               "active-db-name should be updated to the new worksheet's db-name")
           ;; Give the async restore a moment to settle before next test
           (js/setTimeout (fn [] (done)) 50))))

;;; ==========================================================================
;;; In-Memory Re-Frame / Re-Posh / DataScript Cycle
;;; ==========================================================================

(deftest ^:integration conn-init-test
  (testing "create-conn produces a valid connection"
    (let [conn (make-conn!)]
      (is (some? conn))
      (is (d/conn? conn))
      (is (identical? conn @s/conn)))))

(deftest ^:integration re-posh-transact-test
  (testing "re-posh :ds/transact writes datoms readable via d/q"
    (make-conn!)
    (rf/dispatch-sync [:ds/transact [[{:worksheet/uuid "ws-1"
                                       :worksheet/name "Test Worksheet"}]]])
    (let [result (d/q '[:find ?name .
                        :in $
                        :where
                        [?e :worksheet/uuid "ws-1"]
                        [?e :worksheet/name ?name]]
                      @@s/conn)]
      (is (= "Test Worksheet" result)))))

(deftest ^:integration re-posh-subscription-test
  (testing "re-posh subscriptions react to transactions"
    (let [conn (make-conn!)]
      (rp/reg-query-sub
       :test/worksheet-names
       '[:find [?name ...]
         :in $
         :where [_ :worksheet/name ?name]])

      (let [*names (rf/subscribe [:test/worksheet-names])]
        (is (empty? @*names))
        (rf/dispatch-sync [:ds/transact [[{:worksheet/uuid "ws-a"
                                           :worksheet/name "Alpha"}]]])
        (is (= ["Alpha"] @*names))
        (rf/dispatch-sync [:ds/transact [[{:worksheet/uuid "ws-b"
                                           :worksheet/name "Beta"}]]])
        (is (= #{"Alpha" "Beta"} (set @*names)))
        ;; Clear subs before teardown to prevent stale evaluation
        (rf/clear-subscription-cache!)))))

(deftest ^:integration worksheet-new-event-test
  (testing ":worksheet/new creates a worksheet entity with correct attributes"
    (make-conn!)
    (let [ws-uuid "test-new-ws"]
      (rf/dispatch-sync [:worksheet/new {:ws-uuid ws-uuid
                                         :ws-name "New Worksheet"
                                         :modules [:surface]
                                         :version "1.0.0"}])
      (let [*ws (rf/subscribe [:worksheet ws-uuid])]
        (is (some? @*ws))
        (is (= ws-uuid (:worksheet/uuid @*ws)))
        (is (= "New Worksheet" (:worksheet/name @*ws)))))))

(deftest ^:integration transact-many-test
  (testing ":ds/transact-many applies multiple datoms"
    (make-conn!)
    (rf/dispatch-sync [:ds/transact-many
                       [{:worksheet/uuid "ws-multi-1" :worksheet/name "First"}
                        {:worksheet/uuid "ws-multi-2" :worksheet/name "Second"}]])
    (let [count' (d/q '[:find (count ?e) .
                        :where [?e :worksheet/uuid _]]
                      @@s/conn)]
      (is (= 2 count')))))

(deftest ^:integration entity-navigation-test
  (testing "d/entity and d/pull work on the absurder-sql conn"
    (make-conn!)
    (d/transact! @s/conn [{:worksheet/uuid "ws-entity"
                           :worksheet/name "Entity Test"}])
    (let [eid    (d/entid @@s/conn [:worksheet/uuid "ws-entity"])
          entity (d/entity @@s/conn eid)
          pulled (d/pull @@s/conn '[:worksheet/name] eid)]
      (is (some? eid))
      (is (= "Entity Test" (:worksheet/name entity)))
      (is (= {:worksheet/name "Entity Test"} pulled)))))

(deftest ^:integration input-group-round-trip-test
  (testing "add input group, upsert variable, query back via subs"
    (make-conn!)
    (let [ws-uuid "ws-input-rt"]
      (rf/dispatch-sync [:worksheet/new {:ws-uuid ws-uuid
                                         :ws-name "Input RT"
                                         :modules [:surface]}])
      (rf/dispatch-sync [:worksheet/add-input-group ws-uuid "group-1" 0])

      (let [*ws (rf/subscribe [:worksheet ws-uuid])
            igs (:worksheet/input-groups @*ws)]
        (is (= 1 (count igs)))
        (is (= "group-1" (:input-group/group-uuid (first igs))))

        (rf/dispatch-sync [:worksheet/upsert-input-variable
                           ws-uuid "group-1" 0 "gv-1" "42" "ch/h"])
        (let [inputs (->> @*ws
                          :worksheet/input-groups
                          first
                          :input-group/inputs)]
          (is (= 1 (count inputs)))
          (is (= "42" (:input/value (first inputs)))))))))

;;; ==========================================================================
;;; SQLite Storage Round-Trip (Rust persistence)
;;; ==========================================================================

(deftest ^:integration sqlite-init-test
  (async done
         (-> (sql/init!)
             (p/then (fn [_]
                       (is true "sql/init! resolved successfully")
                       (done)))
             (p/catch (fn [e]
                        (is false (str "sql/init! failed: " (.-message e)))
                        (done))))))

(deftest ^:integration sqlite-create-and-restore-test
  (async done
         (let [db-name (str "test-" (random-uuid) ".db")]
           (-> (pss/ensure-initialized!)
               (p/then (fn [_] (sql/init!)))
               (p/then (fn [_] (sql/connect! db-name)))
               (p/then (fn [_sql-conn]
                         ;; Create Rust DB, transact, store via storeToLegacy
                         (let [js-schema (impl-rust/schema->js test-schema)
                               rdb       (.emptyDb WasmDataScript js-schema)]
                           (.transact rdb (pr-str [{:worksheet/uuid "ws-sqlite"
                                                    :worksheet/name "SQLite Test"}]))
                           (.storeToLegacy rdb db-name)
                           ;; Restore via restoreFromLegacy
                           (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                             (is (some? restored) "restored DB should not be nil")
                             (let [cljs-db (impl-rust/sync-from-rust restored)
                                   name'   (d/q '[:find ?name .
                                                  :where
                                                  [?e :worksheet/uuid "ws-sqlite"]
                                                  [?e :worksheet/name ?name]]
                                                cljs-db)]
                               (is (= "SQLite Test" name')
                                   "data should survive save/restore round-trip"))))))
               (p/then (fn [_] (done)))
               (p/catch (fn [e]
                          (is false (str "sqlite round-trip failed: " (.-message e)))
                          (done)))))))

(deftest ^:integration sqlite-export-import-test
  (async done
         (let [db-name (str "test-export-" (random-uuid) ".db")]
           (-> (pss/ensure-initialized!)
               (p/then (fn [_] (sql/init!)))
               (p/then (fn [_] (sql/connect! db-name)))
               (p/then (fn [sql-conn]
                         ;; Create Rust DB, transact, store
                         (let [js-schema (impl-rust/schema->js test-schema)
                               rdb       (.emptyDb WasmDataScript js-schema)]
                           (.transact rdb (pr-str [{:worksheet/uuid "ws-export"
                                                    :worksheet/name "Export Test"}
                                                   {:worksheet/uuid "ws-export-2"
                                                    :worksheet/name "Second WS"}]))
                           (.storeToLegacy rdb db-name)
                           ;; Export bytes
                           (-> (sql/export! sql-conn)
                               (p/then (fn [db-bytes]
                                         (is (pos? (.-length db-bytes))
                                             "exported bytes should be non-empty")
                                         (sql/close! sql-conn)
                                         db-bytes))))))
               ;; Import into a new database
               (p/then (fn [db-bytes]
                         (let [import-name (str "test-import-" (random-uuid) ".db")]
                           (-> (sql/connect! import-name)
                               (p/then (fn [tmp-conn]
                                         (-> (sql/import! tmp-conn db-bytes)
                                             (p/then (fn [_] (sql/close! tmp-conn))))))
                               (p/then (fn [_] (sql/connect! import-name)))
                               (p/then (fn [_sql-conn2]
                                         (let [restored (.restoreFromLegacy WasmDataScript import-name)]
                                           (is (some? restored) "restoreFromLegacy should return a DB")
                                           (let [cljs-db (impl-rust/sync-from-rust restored)
                                                 names   (d/q '[:find [?name ...]
                                                                :where [_ :worksheet/name ?name]]
                                                              cljs-db)]
                                             (is (= #{"Export Test" "Second WS"} (set names))
                                                 "imported DB should contain both worksheets")))))))))
               (p/then (fn [_] (done)))
               (p/catch (fn [e]
                          (is false (str "export/import failed: " (.-message e)))
                          (done)))))))

(deftest ^:integration sqlite-with-re-posh-test
  (async done
         (let [db-name (str "test-reposh-" (random-uuid) ".db")]
           (-> (pss/ensure-initialized!)
               (p/then (fn [_] (sql/init!)))
               (p/then (fn [_] (sql/connect! db-name)))
               (p/then (fn [_sql-conn]
                         ;; Create Rust DB, transact, store
                         (let [js-schema (impl-rust/schema->js test-schema)
                               rdb       (.emptyDb WasmDataScript js-schema)]
                           (.transact rdb (pr-str [{:worksheet/uuid "ws-reposh"
                                                    :worksheet/name "RePosh SQLite"}]))
                           (.storeToLegacy rdb db-name)

                           ;; Sync Rust -> CLJS for posh reactivity
                           (let [cljs-db (impl-rust/sync-from-rust rdb)
                                 conn    (d/conn-from-datoms (d/datoms cljs-db :eavt) test-schema)]
                             (reset! s/conn conn)
                             (rp/connect! conn)
                             (re/init! conn)
                             (impl-rust/set-named-db! "$ws" rdb @conn)

                             ;; Verify query works
                             (let [name' (d/q '[:find ?name .
                                                :where
                                                [_ :worksheet/uuid "ws-reposh"]
                                                [_ :worksheet/name ?name]]
                                              @@s/conn)]
                               (is (= "RePosh SQLite" name')))

                             ;; Restore from storage and verify data survived
                             (let [restored (.restoreFromLegacy WasmDataScript db-name)]
                               (is (some? restored) "restoreFromLegacy should return a DB")
                               (let [restored-db   (impl-rust/sync-from-rust restored)
                                     restored-conn (d/conn-from-datoms (d/datoms restored-db :eavt) test-schema)]
                                 ;; Teardown old, set up restored
                                 (reset! rpdb/store nil)
                                 (rf/clear-subscription-cache!)
                                 (reset! s/conn restored-conn)
                                 (rp/connect! restored-conn)
                                 (re/init! restored-conn)

                                 ;; Verify data survived
                                 (let [name' (d/q '[:find ?name .
                                                    :where
                                                    [_ :worksheet/uuid "ws-reposh"]
                                                    [_ :worksheet/name ?name]]
                                                  @@s/conn)]
                                   (is (= "RePosh SQLite" name')
                                       "data survives re-posh + SQLite round-trip"))

                                 ;; Verify subscription works on restored conn
                                 (let [*ws (rf/subscribe [:worksheet "ws-reposh"])]
                                   (is (some? @*ws))
                                   (is (= "RePosh SQLite" (:worksheet/name @*ws))))))))))
               (p/then (fn [_] (done)))
               (p/catch (fn [e]
                          (is false (str "re-posh + sqlite failed: " (.-message e)))
                          (done)))))))
