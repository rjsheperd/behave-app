(ns behave.worksheet.subs
  (:require [re-frame.core :as rf]
            [re-posh.core  :as rp]))

; Retrieve all worksheet UUID's
(rp/reg-sub
 :worksheet/all
 (fn [_ _]
   {:type  :query
    :query '[:find  ?created ?uuid
             :where [?ws :worksheet/uuid ?uuid]
                    [?ws :worksheet/created ?created]]}))

; Retrieve latest worksheet UUID
(rf/reg-sub
 :worksheet/latest
 (fn [_]
   (rf/subscribe [:worksheet/all]))
 (fn [all-worksheets [_]]
   (last (last (sort-by first all-worksheets)))))

; Retrieve latest worksheet UUID
(rp/reg-sub
 :worksheet/modules
 (fn [_ [_ ws-uuid]]
   {:type  :query
    :query '[:find  [?modules ...]
             :in    $ ?ws-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/modules ?modules]]
    :variables [ws-uuid]}))

; Get state of a particular output
(rp/reg-sub
 :worksheet/output-enabled?
 (fn [_ [_ ws-uuid variable-uuid]]
   {:type  :query
    :query '[:find  [?enabled]
             :in    $ ?ws-uuid ?var-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/outputs ?o]
                    [?o :output/group-variable-uuid ?var-uuid]
                    [?o :output/enabled? ?enabled]]
    :variables [ws-uuid variable-uuid]}))

; Get the value of a particular input
(rp/reg-sub
 :worksheet/input
 (fn [_ [_ ws-uuid group-uuid repeat-id group-variable-uuid]]
   {:type  :query
    :query '[:find  [?value]
             :in    $ ?ws-uuid ?group-uuid ?repeat-id ?group-var-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/input-groups ?g]
                    [?g :input-group/group-uuid ?group-uuid]
                    [?g :input-group/repeat-id ?repeat-id]
                    [?g :input-group/inputs ?i]
                    [?i :input/group-variable-uuid ?group-var-uuid]
                    [?i :input/value ?value]]
    :variables [ws-uuid group-uuid repeat-id group-variable-uuid]}))

; Find groups matching a group-uuid
(rp/reg-sub
 :worksheet/repeat-groups
 (fn [_ [_ ws-uuid group-uuid]]
   {:type  :query
    :query '[:find  [?g ...]
             :in    $ ?ws-uuid ?group-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/input-groups ?g]
                    [?g :input-group/group-uuid ?group-uuid]]
    :variables [ws-uuid group-uuid]}))

(rf/reg-sub
 :worksheet/all-inputs
 (fn [_ [_ ws-uuid]]
   (let [inputs @(rf/subscribe [:query
                                '[:find  ?group-uuid ?repeat-id ?group-var-uuid ?value
                                    :in    $ ?ws-uuid
                                    :where [?w :worksheet/uuid ?ws-uuid]
                                            [?w :worksheet/input-groups ?g]
                                            [?g :input-group/group-uuid ?group-uuid]
                                            [?g :input-group/repeat-id ?repeat-id]
                                            [?g :input-group/inputs ?i]
                                            [?i :input/group-variable-uuid ?group-var-uuid]
                                            [?i :input/value ?value]]
                                [ws-uuid]])]
     (reduce (fn [acc [group-uuid repeat-id group-var-uuid value]]
               (assoc-in acc [group-uuid repeat-id group-var-uuid] value))
             {}
             inputs))))

(rp/reg-sub
 :worksheet/all-outputs
 (fn [_ [_ ws-uuid]]
   {:type  :query
    :query '[:find  [?uuid ...]
             :in    $ ?ws-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/outputs ?o]
                    [?o :output/group-variable-uuid ?uuid]
                    [?o :output/enabled? true]]
    :variables [ws-uuid]}))

(rp/reg-sub
 :worksheet/_results-table
 (fn [_ [_ ws-uuid]]
   {:type  :query
    :query '[:find  [?t ...]
             :in    $ ?ws-uuid
             :where [?w :worksheet/uuid ?ws-uuid]
                    [?w :worksheet/result-table ?t]]
    :variables [ws-uuid]}))

(rp/reg-sub
 :worksheet/_results-table-headers
 (fn [[_ ws-uuid]]
   (rf/subscribe [:worksheet/_results-table ws-uuid]))
 (fn [tables _]
   (let [table (first tables)]
     {:type  :query
      :query '[:find  [?h ...]
               :in    $ ?t
               :where [?t :result-table/headers ?h]]
      :variables [table]})))

(rp/reg-sub
 :worksheet/results-table-headers
 (fn [[_ ws-uuid]]
   (rf/subscribe [:worksheet/_results-table-headers ws-uuid]))
 (fn [headers _]
   {:type    :pull-many
    :pattern '[*]
    :ids     headers}))

(rp/reg-sub
 :worksheet/_results-table-rows
 (fn [[_ ws-uuid]]
   (rf/subscribe [:worksheet/_results-table ws-uuid]))
 (fn [tables _]
   {:type  :query
    :query '[:find  [?r ...]
             :in    $ ?t
             :where [?t :result-table/rows ?r]]
    :variables [(first tables)]}))

(rp/reg-sub
 :worksheet/results-table-rows
 (fn [[_ ws-uuid]]
   (rf/subscribe [:worksheet/_results-table-rows ws-uuid]))
 (fn [rows _]
   {:type    :pull-many
    :pattern '[*]
    :ids     rows}))

(rp/reg-sub
 :worksheet/_results-table-cells
 (fn [[_ ws-uuid _]]
   (rf/subscribe [:worksheet/_results-table ws-uuid]))
 (fn [tables [_ _ row-id]]
   {:type  :query
    :query '[:find  [?c ...]
             :in    $ ?t ?row-id
             :where [?t :result-table/rows ?r]
                    [?r :result-row/id ?row-id]
                    [?r :result-row/cells ?c]]
    :variables [(first tables) row-id]}))

(rp/reg-sub
 :worksheet/results-table-cells
 (fn [[_ ws-uuid row-id]]
   (rf/subscribe [:worksheet/_results-table-cells ws-uuid row-id]))
 (fn [cells _]
   {:type    :pull-many
    :pattern '[* {:result-cell/header [*]}]
    :ids     cells}))

(rp/reg-sub
 :worksheet/_results-table-column
 (fn [[_ ws-uuid _]]
   (rf/subscribe [:worksheet/_results-table ws-uuid]))
 (fn [tables [_ _ group-variable-uuid]]
   {:type  :query
    :query '[:find  [?c ...]
             :in    $ ?t ?group-variable-uuid
             :where [?t :result-table/headers ?h]
                    [?h :result-header/group-variable-uuid ?group-variable-uuid]
                    [?c :result-cell/header ?h]]
    :variables [(first tables) group-variable-uuid]}))

(rp/reg-sub
 :worksheet/results-table-column
 (fn [[_ ws-uuid row-id]]
   (rf/subscribe [:worksheet/_results-table-column ws-uuid row-id]))
 (fn [cells _]
   {:type    :pull-many
    :pattern '[* {:result-cell/header [*]}]
    :ids     cells}))
