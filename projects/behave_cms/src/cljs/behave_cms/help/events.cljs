(ns behave-cms.help.events
  (:require [re-frame.core :as rf :refer [path]]))

;;; Editor

(rf/reg-event-db
 :help-editor/set
 (path :state :editors :help-page)
 (fn [page [_ help-key k v]]
   (assoc-in page [help-key k] v)))

;; Saving to DataScript

(rf/reg-event-fx
 :help-editor/save
 (fn [{db :db} [_ help-key latest-help-page]]
   (let [edited-page (get-in db [:state :editors :help-page help-key])
         language    (:language edited-page)
         event       (if (:db/id latest-help-page)
                       :api/update-entity
                       :api/create-entity)
         data        (merge (select-keys latest-help-page [:db/id])
                            (select-keys edited-page [:help-page/content])
                            {:help-page/key help-key :language/_help-page language})]
     {:fx [[:dispatch [event data]]]})))
