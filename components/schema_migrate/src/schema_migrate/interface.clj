(ns schema-migrate.interface
  (:require [schema-migrate.core :as c]))

(def ^{:argslist '([conn t])
       :doc      "Get the :bp/uuid using translation-key"}
  t-key->uuid c/t-key->uuid)

(def ^{:argslist '([conn t])
       :doc      "Get the datomic entity using translation-key"}
  t-key->enitty c/t-key->entity)

(def ^{:argslist '([conn t])
       :doc      "Get the :db/id using translation-key"}
  t-key->eid c/t-key->eid)

(def ^{:argslist '([conn attr])
       :doc      "Sets :db/isComponent true for a given schema attribute.
                  Takes a datahike conn."}
  make-attr-is-component! c/make-attr-is-component!)

(def ^{:argslist '([conn attr])
       :doc      "Returns the payload for making a schema attribute a \"Component\""}
  make-attr-is-component-payload c/make-attr-is-component-payload)

(def ^{:argslist '([conn attr])
       :doc      "Sets :db/isComponent true for a given schema attribute.
                  Takes a datahike conn."}
  make-attr-is-component! c/make-attr-is-component!)

(def ^{:argslist '([conn tx])
       :doc      "Given a transaction ID or a transaction result (return from datomic.api/transact),
                  Reassert retracted datoms and retract asserted datoms in a transaction,
                  effectively \"undoing\" the transaction."}
  rollback-tx! c/rollback-tx!)

(def ^{:argslist '([])
       :doc      "Generate random UUID as a string."}
  rand-uuid c/rand-uuid)

(def ^{:argslist '([db uuid])
       :doc      "Convert a UUID to an entity ID."}
  uuid->id c/uuid->id)

(def ^{:argslist '([db nid])
       :doc      "Convert a Nano-ID to an entity ID."}
  nid->id c/nid->id)

(def ^{:argslist '([db nid])
       :doc      "Convert a Nano-ID to an entity's UUID."}
  nid->uuid c/nid->uuid)
