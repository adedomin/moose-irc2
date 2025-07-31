# Moose2 IRC bot

Running
=======

```sh
moose-irc2 -c /etc/moose-irc2/NETWORK_NAME.json init
# now edit /etc/moose-irc2/NETWORK_NAME.json
# if you want invites, edit invite-file in the config.json
cp -t /etc/systemd/system ./contrib/etc/systemd/system/moose-irc2.service ./contrib/etc/systemd/system/moose-irc2@.service
systemctl daemon-reload
systemctl enable --now moose-irc2.service
systemctl enable --now moose-irc2@NETWORK_NAME.service
# you can stop all bot instances via moose-irc2.service
systemctl stop moose-irc2.service
```
