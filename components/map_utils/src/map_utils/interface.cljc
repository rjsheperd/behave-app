(ns map-utils.interface
  (:require [map-utils.core :as c]))

(def ^{:arglist '([k coll])
       :doc      "Returns a map where `coll` is indexed by key `k`.
                  WARNING: Will not work with multiple entries with the same value for key `k`. For that, use `group-by`."}
  index-by c/index-by)

(def ^{:arglist '([map key val])
       :doc      "Associate a key with a value in a map. If the key already exists in the map,
                 a vector of values is associated with the key."}
  assoc-conj c/assoc-conj)

(def ^{:arglist '([f coll])
       :doc     "A version of `filter` that uses transients."}
  filterm c/filterm)

(def ^{:arglist '([f coll])
       :doc     "A version of `map` that uses transients."}
  mapm c/mapm)

(def ^{:arglist '([])
       :doc     "Creates a sorted-map where the keys are sorted in reverse order."}
  reverse-sorted-map c/reverse-sorted-map)

(def ^{:arglist '([old-map new-map])
       :doc     "Takes in two maps with the same keys and determines which values are different between the two maps and returns the keys associated with the changed values."}
  get-changed-keys c/get-changed-keys)

(def ^{:arglist '([v coll])
       :doc     "Returns the two values from a sorted collection that bound v."}
  find-boundary-values c/find-boundary-values)

(def ^{:arglist '([coll id] [coll id k])
       :doc     "Finds the value of a key by id if one exists."}
  find-key-by-id c/find-key-by-id)

(def ^{:arglist '([coll id])
       :doc     "Finds the value of a specific id if one exists."}
  find-by-id c/find-by-id)
