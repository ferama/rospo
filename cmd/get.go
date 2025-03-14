package cmd

import (
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"strings"
	"time"

	pb "github.com/cheggaaa/pb/v3"
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/rio"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/pkg/sftp"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(getCmd)

	cmnflags.AddSshClientFlags(getCmd.Flags())
	getCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")
}

func getFile(client *sftp.Client, remote, localPath string) error {
	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}
	remoteStat, err := client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	rFile, err := client.Open(remotePath)
	if err != nil {
		return fmt.Errorf("cannot open remote file for read: %s", err)
	}
	defer rFile.Close()

	localStat, err := os.Stat(localPath)
	if err == nil && localStat.IsDir() {
		localPath = filepath.Join(localPath, filepath.Base(remotePath))
	}

	var offset int64
	lFile, err := os.OpenFile(localPath, os.O_CREATE|os.O_WRONLY, 0644)
	if err == nil {
		// Check if the remote file already exists and get its size
		offset, err = lFile.Seek(0, io.SeekEnd)
		if err != nil {
			return fmt.Errorf("cannot seek remote file: %s", err)
		}
		lFile.Close()
	} else {
		offset = 0
	}

	// Reopen the local file for writing from the offset
	lFile, err = os.OpenFile(localPath, os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return fmt.Errorf("cannot open remote file for write: %s", err)
	}
	defer rFile.Close()

	// Seek the remote file to the offset
	_, err = rFile.Seek(offset, io.SeekStart)
	if err != nil {
		return fmt.Errorf("cannot seek remote file: %s", err)
	}

	// Seek the local file to the offset
	_, err = lFile.Seek(offset, io.SeekStart)
	if err != nil {
		return fmt.Errorf("cannot seek local file: %s", err)
	}

	byteswrittench := make(chan int64)
	go func() {
		tmpl := `{{string . "target" | white}} {{with string . "prefix"}}{{.}} {{end}}{{counters . | blue }} {{bar . "|" "=" (cycle . "↖" "↗" "↘" "↙" ) "." "|" }} {{percent . | blue }} {{speed . | blue }} {{rtime . "ETA %s" | blue }}{{with string . "suffix"}} {{.}}{{end}}`
		pbar := pb.ProgressBarTemplate(tmpl).Start(0)
		pbar.Set(pb.Bytes, true)
		pbar.Set(pb.SIBytesPrefix, true)

		pbar.Set("target", filepath.Base(remotePath))
		pbar.SetTotal(remoteStat.Size())
		for w := range byteswrittench {
			pbar.Add64(w)
		}
		pbar.Finish()
	}()
	byteswrittench <- offset
	err = rio.CopyBuffer(lFile, rFile, byteswrittench)
	close(byteswrittench)
	if err != nil {
		return fmt.Errorf("error while writing local file: %s", err)
	}
	lFile.Chmod(remoteStat.Mode())
	return nil
}

func retryGetFile(conn *sshc.SshConnection, remote, local string) {
	var err error = nil
	var client *sftp.Client
	for {
		if err != nil {
			time.Sleep(1 * time.Second)
		}

		client, err = sftp.NewClient(conn.Client)
		if err != nil {
			log.Printf("err while connecting: %s", err)
			continue

		}
		defer client.Close()
		if remote == "" {
			remote, err = client.Getwd()
			if err != nil {
				log.Printf("remote is empty and I can get cwd, %s", err)
				continue
			}
		}

		err = getFile(client, remote, local)
		if err != nil {
			log.Printf("error while copying file: %s", err)
			continue
		} else {
			break
		}
	}
}

func getFileRecursive(conn *sshc.SshConnection, remote, local string) error {
	client, err := sftp.NewClient(conn.Client)
	if err != nil {
		log.Printf("err while connecting: %s", err)
		return err

	}
	defer client.Close()

	remotePath, err := client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	remoteStat, err := client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("remote path is not a directory: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	dir := filepath.Dir(remotePath)
	walker := client.Walk(remotePath)
	for walker.Step() {
		if walker.Err() != nil {
			log.Println(walker.Err())
			continue
		}
		remotePath := walker.Path()
		stat := walker.Stat()
		part := strings.TrimPrefix(remotePath, dir)
		localPath := filepath.Join(local, part)
		if stat.IsDir() {
			err := os.Mkdir(localPath, stat.Mode())
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", localPath, err)
			}
		} else {
			retryGetFile(conn, remotePath, localPath)
		}
	}
	return nil
}

var getCmd = &cobra.Command{
	Use:   "get [user@]host[:port] remote [local]",
	Short: "Gets a file from remote",
	Long:  "Gets a file from remote",
	Example: `
  # downloads a file from the remote server
  $ rospo get myserver:2222 file.txt .

  # downloads recursively all contents of myremotefolder to local current working directory
  $ rospo get myserver:2222 /home/myuser/myremotefolder -r

  # downloads recursively all contents of myremotefolder to local target directory
  $ rospo get myserver:2222 /home/myserver/myremotefolder ~/mylocalfolder -r
	`,
	Args: cobra.MinimumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		remote := args[1]
		local := ""
		if len(args) > 2 {
			local = args[2]
		}
		recursive, _ := cmd.Flags().GetBool("recursive")
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		sshcConf.Quiet = true
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()
		conn.ReadyWait()

		if recursive {
			err := getFileRecursive(conn, remote, local)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		} else {
			retryGetFile(conn, remote, local)
		}

	},
}
