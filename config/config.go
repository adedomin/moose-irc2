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
	_ "embed"
	"log"
	"os"
	"strings"
	"sync"
)

type Config struct {
	Nick       string
	Pass       string
	Host       string
	Sasl       string
	Tls        bool
	IgnoreBots bool     `json:"ignore-bots"`
	SendDelay  Duration `json:"send-delay"`
	MooseUrl   string   `json:"moose-url"`

	InviteFile string `json:"-"`

	Channels       []string
	InviteChannels sync.Map `json:"-"`
}

var C Config

func init() {
	configPath := ""
	inC := false
	inI := false
	initMode := false
	for _, v := range os.Args {
		switch v {
		case "-c", "--config":
			inC = true
		case "-i", "--invitefile":
			inI = true
		case "-h", "--help":
			usage()
		default:
			if inC {
				inC = false
				configPath = v
			} else if inI {
				inI = false
				C.InviteFile = v
			} else if v == "init" {
				initMode = true
			} else if v == "help" {
				usage()
			}
		}
	}

	if initMode {
		createConfig(configPath)
	}

	openConfig(configPath)

	if C.InviteFile != "" {
		openAndLoadInvites(C.InviteFile)
	}
}

//go:embed example.json
var exampleConfig []byte

func createConfig(userPath string) {
	var file *os.File
	var err error

	if userPath == "" {
		usage1("Error: You must explicitly give a configuration.")
	}
	ensureDirToFile(userPath)
	file, err = os.Create(userPath)
	if err != nil {
		log.Fatalf("Error: Failed to create config: %s", err)
	}
	defer file.Close()

	if _, err = file.Write(exampleConfig); err != nil {
		log.Fatalf("Error: Failed to create config: %s", err)
	} else {
		log.Printf("Ok: Created Configuration at: %s", userPath)
		os.Exit(0)
	}
}

func SplitChannelList(channels []string) string {
	lineSize := 0
	first := true

	var ret strings.Builder

	// Splits configured list of channels into a safe set of commands
	for _, channel := range channels {
		if lineSize+len(channel) > 510 {
			lineSize = 0
			first = true
			ret.WriteByte('\r')
			ret.WriteByte('\n')
		}

		if !first {
			ret.WriteByte(',')
			lineSize += 1
		} else {
			ret.WriteString("JOIN ")
			lineSize += 5
			first = false
		}
		ret.WriteString(channel)
		lineSize += len(channel)
	}
	ret.WriteByte('\r')
	ret.WriteByte('\n')

	return ret.String()
}

func usage1(failure string) {
	if failure != "" {
		println(failure)
		println("")
	}
	usage()
}

func usage() {
	println("Usage: moose-irc2 [init] -c config [-i invite-file]")
	println("")
	println("  init       Initialize configuration for the bot.")
	println("  -c config  Use config given on the command line.")
	println("  -i invite  Accept invites and save them for future restarts.")
	println("")
	println("moose-irc2 requires an explicit configuration path.")
	println("It is intended to be invoked separately for each irc NETWORK.")
	println("e.g. systemctl start moose-irc2@NETWORK.service")
	println("where the ExecStart value looks something like:")
	println("  /some/path/moose-irc2 -c /some/path/%i.json -i /some/path/%i.json")
	os.Exit(1)
}
