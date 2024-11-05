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
package botstuff

import "strings"

const (
	mInvalid = 0
	mIrc     = iota
	mImg     = iota
	mSearch  = iota
	mBots    = iota
	mHelp    = iota
)

const (
	findCommand = iota
	findArgs    = iota
	findMoose   = iota
)

type command struct {
	cmd   int
	moose string
}

func newCommand() command {
	return command{
		cmd:   mInvalid,
		moose: "random",
	}
}

func parseMooseArgs(cmdline string) command {
	tokens := strings.Split(cmdline, " ")
	ret := newCommand()
	parseState := findCommand
loop:
	for pos, val := range tokens {
		switch parseState {
		case findCommand:
			switch val {
			case ".moose", "!moose", "moose", ".mooseme", "!mooseme", "mooseme":
				ret.cmd = mIrc
				parseState = findArgs
			case ".mooseimg", "!mooseimg", "mooseimg":
				ret.cmd = mImg
				parseState = findArgs
			case ".moosesearch", "!moosesearch", "moosesearch":
				ret.cmd = mSearch
				parseState = findArgs
			case ".bots", "!bots", ".help", "!help":
				ret.cmd = mBots
				break loop
			default:
				ret.cmd = mInvalid
				break loop // Invalid command.
			}
		case findArgs:
			ret.moose = ""
			switch val {
			case "":
			case "--":
				parseState = findMoose
			case "-h", "--help":
				ret.cmd = mHelp
				break loop
			case "-s", "--search":
				ret.cmd = mSearch
				parseState = findMoose
			case "-i", "--image":
				ret.cmd = mImg
				parseState = findMoose
			case "-r", "--random":
				ret.moose = "random"
				break loop
			case "-l", "--latest":
				ret.moose = "latest"
				break loop
			case "-o", "--oldest":
				ret.moose = "oldest"
				break loop
			default:
				ret.moose = strings.Join(tokens[pos:], " ")
				break loop
			}
		case findMoose:
			switch val {
			case "":
			default:
				ret.moose = strings.Join(tokens[pos:], " ")
				break loop
			}
		}
	}
	ret.moose = strings.TrimSpace(ret.moose)
	return ret
}
