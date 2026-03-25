(ns absurder-sql.datascript.conn
  (:require
   [absurder-sql.datascript.db :as db #?@(:cljs [:refer [DB FilteredDB]])]
   [extend-clj.core :as extend]
   [#?(:clj me.tonsky.persistent-sorted-set :cljs absurder-sql.datascript.persistent-sorted-set) :as set])
  #?(:clj
     (:import
      [absurder-sql.datascript.db DB FilteredDB])))

(extend/deftype-atom Conn [atom]
  (deref-impl [this]
              (:db @atom))
  (compare-and-set-impl [this oldv newv]
                        (compare-and-set!
                         atom
                         (assoc @atom :db oldv)
                         (assoc @atom :db newv))))

(defn- make-conn [opts]
  (->Conn (atom opts)))

(defn with
  ([db tx-data] (with db tx-data nil))
  ([db tx-data tx-meta]
   {:pre [(db/db? db)]}
   (if (instance? FilteredDB db)
     (throw (ex-info "Filtered DB cannot be modified" {:error :transaction/filtered}))
     (db/transact-tx-data (db/->TxReport db db [] {} tx-meta) tx-data))))

(defn ^DB db-with
  "Applies transaction to an immutable db value, returning new immutable db value. Same as `(:db-after (with db tx-data))`."
  [db tx-data]
  {:pre [(db/db? db)]}
  (:db-after (with db tx-data)))

(defn conn? [conn]
  (and
   #?(:clj (instance? clojure.lang.IDeref conn)
      :cljs (satisfies? cljs.core/IDeref conn))
   (if-some [db @conn]
     (db/db? db)
     true)))

(defn conn-from-db [db]
  {:pre [(db/db? db)]}
  (make-conn {:db db}))

(defn conn-from-datoms
  ([datoms]
   (conn-from-db (db/init-db datoms nil {})))
  ([datoms schema]
   (conn-from-db (db/init-db datoms schema {})))
  ([datoms schema opts]
   (conn-from-db (db/init-db datoms schema opts))))

(defn create-conn
  ([]
   (conn-from-db (db/empty-db nil {})))
  ([schema]
   (conn-from-db (db/empty-db schema {})))
  ([schema opts]
   (conn-from-db (db/empty-db schema opts))))

(defn ^:no-doc -transact! [conn tx-data tx-meta]
  {:pre [(conn? conn)]}
  (let [*report (volatile! nil)]
    (swap! conn
           (fn [db]
             (let [r (with db tx-data tx-meta)]
               (vreset! *report r)
               (:db-after r))))
    @*report))

(defn transact!
  ([conn tx-data]
   (transact! conn tx-data nil))
  ([conn tx-data tx-meta]
   {:pre [(conn? conn)]}
   (locking conn
     (let [report (-transact! conn tx-data tx-meta)]
       (doseq [[_ callback] (:listeners @(:atom conn))]
         (callback report))
       report))))

(defn reset-conn!
  ([conn db]
   (reset-conn! conn db nil))
  ([conn db tx-meta]
   {:pre [(conn? conn)
          (db/db? db)]}
   (let [db-before @conn
         report    (db/map->TxReport
                    {:db-before db-before
                     :db-after  db
                     :tx-data   (concat
                                 (when db-before
                                   (map #(assoc % :added false) (db/-datoms db-before :eavt nil nil nil nil)))
                                 (db/-datoms db :eavt nil nil nil nil))
                     :tx-meta   tx-meta})]
     (reset! conn db)
     (doseq [[_ callback] (:listeners @(:atom conn))]
       (callback report))
     db)))

(defn reset-schema! [conn schema]
  {:pre [(conn? conn)]}
  (swap! conn db/with-schema schema))

(defn listen!
  ([conn callback]
   (listen! conn (rand) callback))
  ([conn key callback]
   {:pre [(conn? conn)]}
   (swap! (:atom conn) update :listeners assoc key callback)
   key))

(defn unlisten! [conn key]
  {:pre [(conn? conn)]}
  (swap! (:atom conn) update :listeners dissoc key))
