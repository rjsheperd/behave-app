(ns behave.schema.queries
  (:require [clojure.string :as str]
            #?(:cljs [datascript.core :as d]
               :clj  [datahike.api :as d])))

(def rules
  '[
    ;; -- Find an entity's UUID
    [(uuid ?e ?uuid)
     [?e :bp/uuid ?uuid]]

    ;; -- Find an entity's name
    [(name ?e ?name)
      [?e :application/name ?name]]

    [(name ?e ?name)
      [?e :module/name ?name]]

    [(name ?e ?name)
      [?e :submodule/name ?name]]

    [(name ?e ?name)
      [?e :group/name ?name]]

    [(name ?e ?name)
      [?e :group-variable/name ?name]]

    ;; --  Recursive rules to find a group's subgroups 
    [(subgroup ?g ?s)
     [?g :group/children ?s]]

    [(subgroup ?g ?s)
     [?g :group/children ?x]
     (subgroup ?x ?s)]

    ;; --  Recursive rule to find a submodule's groups
    [(group ?s ?g)
     [?m :submodule/groups ?s]]

    [(group ?s ?g)
     [?m :submodule/groups ?x]
     (subgroup ?x ?g)]

    ;; --  Submodule of a module
    [(submodule ?m ?s)
     [?m :module/submodules ?s]]

    ;; --  Application's modules
    [(module ?a ?m)
     [?e :application/modules ?m]]

    ;; --  Group's group-variables
    [(variable ?g ?v)
     [?g :group/group-variables ?v]]

    ;; -- Entity's Input/Ouput (Group Variable, Group, Submodule)
    [(io ?e ?io)
     [?e :submodule/io ?io]]
    
    [(io ?e ?io)
     (group ?s ?e)
     [?e :submodule/io ?io]]

    [(io ?e ?io)
     (variable ?g ?e)
     (group ?s ?g)
     [?e :submodule/io ?io]]

    ;; --  Find the root application for a module, submodule, group, or subgroup
    [(app-root ?a ?g)
     [?sm :submodule/groups ?g]
     [?m :module/submodules ?sm]
     [?a :application/modules ?m]]

    [(app-root ?a ?s)
     (subgroup ?g ?s)
     [?sm :submodule/groups ?g]
     [?m :module/submodules ?sm]
     [?a :application/modules ?m]]

    ;; --  Language of an
    [(language ?code ?l)
     [?l :language/shortcode ?code]]

    ;; --  Entity's translation key
    [(translation-key ?e ?k)
     (or
      [?e :application/translation-key ?k]
      [?e :module/translation-key ?k]
      [?e :submodule/translation-key ?k]
      [?e :group/translation-key ?k]
      [?e :group-variable/translation-key ?k])]

    ;; --  Translation key to translation
    [(translation ?k ?t ?content)
     [?t :translation/key ?k]
     [?t :translation/translation ?content]]

    ;; --  Entity's help key
    [(translation-key ?e ?k)
     (or
      [?e :application/help-key ?k]
      [?e :module/help-key ?k]
      [?e :submodule/help-key ?k]
      [?e :group/help-key ?k]
      [?e :group-variable/help-key ?k])]])

(defn q [query conn & args]
  (apply d/q query conn rules args))
