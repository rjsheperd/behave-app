#!/usr/bin/env sh

today() {
  date +"%Y-%m-%d" 
}

BACKUP="datomic-backup-$(today).dump"

# cd ~/behave-polylith
# eval $(ssh-agent)
# ssh-add ~/.ssh/id_ed25519

echo "Creating backup on Dev"
ssh sig-app@goshawk 'cd behave-polylith && bb dump'

echo "Copying backup from Dev: $BACKUP"
scp sig-app@goshawk:~/behave-polylith/$BACKUP ~/.behave_cms/

echo "Restoring backup: $BACKUP"
bb restore -file ~/.behave_cms/$BACKUP

# echo "Restarting CMS"
# cd ~/behave-polylith/projects/behave_cms
# bb restart

# echo "Restarting App"
# cd ~/behave-polylith/projects/behave
# bb restart
