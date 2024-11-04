/*
 *  BSD 2-Clause License
 *
 *  Copyright (c) 2024, Anthony DeDominic
 *
 *  Redistribution and use in source and binary forms, with or without
 *  modification, are permitted provided that the following conditions are met:
 *
 *  1. Redistributions of source code must retain the above copyright notice, this
 *     list of conditions and the following disclaimer.
 *
 *  2. Redistributions in binary form must reproduce the above copyright notice,
 *     this list of conditions and the following disclaimer in the documentation
 *     and/or other materials provided with the distribution.
 *
 *  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 *  AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 *  IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 *  DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 *  FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 *  DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 *  SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 *  CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 *  OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 *  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
package config

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"log"
	"os"
	"path"
)

func ensureDirToFile(fpath string) {
	if err := os.MkdirAll(path.Dir(fpath), 0700); err != nil {
		log.Printf("Warn: Failed to ensure path to file (%s), see (%s)\n", fpath, err)
	}
}

func openConfig(configPath string) {
	var file *os.File
	var err error
	if configPath == "" {
		usage1("Error: You must explicitly give a configuration.")
	}
	file, err = os.Open(configPath)
	if err != nil {
		log.Printf("Error: Could not open config path.")
		log.Fatalf("- %s", err)
	}
	defer file.Close()

	data, err := io.ReadAll(file)
	if err != nil {
		log.Fatal(err)
	}
	err = json.Unmarshal(data, &C)
	if err != nil {
		log.Fatal(err)
	}
}

func openAndLoadInvites(invitePath string) {
	var file *os.File
	var err error

	if invitePath == "" {
		usage1("Error: You must give a valid invite file path.")
	}
	file, err = os.Open(invitePath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			ensureDirToFile(invitePath)
			file, err = os.Create(invitePath)
			if err != nil {
				log.Fatalf("Error: Could not create missing file: %s", err)
			}
			var cnt int
			cnt, err = file.WriteString("[]")
			file.Seek(0, io.SeekStart)
			if cnt != 2 || err != nil {
				log.Fatalf("Error: Failed to create basic invite file: %s", err)
			}
		} else {
			log.Fatalf("Error: Cannot open invite file (%s) due to %s", invitePath, err)
		}
	}
	defer file.Close()

	var invites []string
	data, err := io.ReadAll(file)
	if err != nil {
		log.Fatal(err)
	}
	err = json.Unmarshal(data, &invites)
	if err != nil {
		log.Fatal(err)
	}
	// init gI
	gI.inviteChannels = make(map[string]struct{}, len(invites))
	for _, val := range invites {
		gI.inviteChannels[val] = struct{}{}
	}

	invites = append(invites, C.Channels...)
	// struct{} is allegedly a Zero-Sized Type
	uniq := make(map[string]struct{}, len(invites))
	for _, val := range invites {
		uniq[val] = struct{}{}
	}
	C.Channels = make([]string, 0, len(invites))
	for key := range uniq {
		C.Channels = append(C.Channels, key)
	}
}

const (
	AddInvite = iota
	DelInvite = iota
)

func SaveNewInvite(action int, newchan string) {
	gI.inviteLock.Lock()
	defer gI.inviteLock.Unlock()

	tDir := path.Dir(C.InviteFile)
	file, err := os.CreateTemp(tDir, ".invite.*.json")
	if err != nil {
		log.Printf("ERROR: Failed to open up temporary invite file: %s", err)
		return
	}
	defer file.Close()
	defer os.Remove(file.Name())

	if action == AddInvite {
		gI.inviteChannels[newchan] = struct{}{}
	} else {
		delete(gI.inviteChannels, newchan)
	}
	failed := false
	// roll-back on error
	defer func() {
		if failed {
			log.Println("ERROR: rolling back state of invites.")
			if action == AddInvite {
				delete(gI.inviteChannels, newchan)
			} else {
				gI.inviteChannels[newchan] = struct{}{}
			}
		}
	}()

	channels := make([]string, 0, len(gI.inviteChannels))
	for key := range gI.inviteChannels {
		channels = append(channels, key)
	}

	b, err := json.Marshal(&channels)
	nwr, err := io.Copy(file, bytes.NewReader(b))
	if err != nil {
		log.Printf("ERROR: Failed to write to temporary invite file: %s", err)
		failed = true
		return
	} else if int(nwr) != len(b) {
		log.Println("ERROR: Failed to fully write to temporary invite file.")
		failed = true
		return
	}
	err = os.Rename(file.Name(), C.InviteFile)
	if err != nil {
		log.Printf("ERROR: Failed to move to temporary invite file over old invite file: %s", err)
		failed = true
		return
	}
	err = file.Sync()
	if err != nil {
		panic("Filesystem is broken. Could not sync.")
	}
}
