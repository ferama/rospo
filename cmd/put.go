package cmd

import (
	"fmt"
	"io/fs"
	"log"
	"os"
	"path/filepath"
	"strings"
	"sync"

	"github.com/fatih/color"
	"github.com/ferama/rospo/cmd/cmnflags"
	"github.com/ferama/rospo/pkg/sshc"
	"github.com/ferama/rospo/pkg/worker"
	"github.com/spf13/cobra"
	"github.com/vbauerster/mpb/v8"
	"github.com/vbauerster/mpb/v8/decor"
)

var putWG sync.WaitGroup
var putProgress = mpb.New(mpb.WithWidth(60), mpb.WithWaitGroup(&putWG))

func init() {
	rootCmd.AddCommand(putCmd)

	cmnflags.AddSshClientFlags(putCmd.Flags())
	putCmd.Flags().IntP("max-workers", "w", 16, "nmber of parallel workers")
	putCmd.Flags().IntP("concurrent-uploads", "c", 4, "concurrent uploads (recursive only)")
	putCmd.Flags().BoolP("recursive", "r", false, "if the copy should be recursive")

}

var putProgressFunc sshc.ProgressFunc = func(fileSize int64, offset int64, fileName string, progressCh chan int64) {
	putWG.Add(1)
	defer putWG.Done()

	var val int64
	val = offset

	pbar := putProgress.AddBar(fileSize,
		mpb.PrependDecorators(
			decor.Name(color.BlueString("⬆ %s ", fileName)),
		),
		mpb.AppendDecorators(
			decor.CountersKibiByte("% .2f / % .2f "), // Human-readable size
			decor.Percentage(decor.WC{W: 5}),         // Percentage with fixed width
		),
		mpb.BarFillerClearOnComplete(),
	)

	for w := range progressCh {
		val += w
		pbar.SetCurrent(val)
	}

	pbar.Completed()
}

func putFileRecursive(sftpConn *sshc.SftpClient, remote, local string, maxWorkers int, concurrent int) error {
	sftpConn.ReadyWait()

	remotePath, err := sftpConn.Client.RealPath(remote)
	if err != nil {
		return fmt.Errorf("invalid remote path: %s", remotePath)
	}

	localStat, err := os.Stat(local)
	if err != nil {
		return fmt.Errorf("cannot stat local path: %s", local)
	}
	if !localStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", local)
	}

	remoteStat, err := sftpConn.Client.Stat(remotePath)
	if err != nil {
		return fmt.Errorf("cannot stat remote path: %s", remotePath)
	}
	if !remoteStat.IsDir() {
		return fmt.Errorf("local path is not a directory: %s", remotePath)
	}

	pool := worker.NewPool(concurrent)

	dir := filepath.Base(local)
	err = filepath.WalkDir(local, func(localPath string, d fs.DirEntry, err error) error {
		part := strings.TrimPrefix(localPath, local)
		targetPath := filepath.Join(remotePath, dir, part)
		if d.IsDir() {
			err := sftpConn.Client.Mkdir(targetPath)
			if err != nil {
				return fmt.Errorf("cannot create directory %s: %s", remotePath, err)
			}
		} else {
			pool.Enqueue(func() {
				sftpConn.PutFile(targetPath, localPath, maxWorkers, &putProgressFunc)
			})
		}
		return nil
	})
	if err != nil {
		log.Println(err)
	}
	pool.Wait()
	return nil
}

var putCmd = &cobra.Command{
	Use:   "put [user@]host[:port] local [remote]",
	Short: "Puts files from local to remote",
	Long:  `Puts files from local to remote`,
	Example: `
  # uploads a file to the remote server
  $ rospo put myserver:2222 ~/mylocalfolder/myfile.txt /home/myuser/

  # uploads recursively all contents of mylocalfolder to remote current working directory
  $ rospo put myserver:2222 ~/mylocalfolder -r

  # uploads recursively all contents of mylocalfolder to remote target directory
  $ rospo put myserver:2222 ~/mylocalfolder /home/myuser/myremotefolder -r
	`,
	Args: cobra.MinimumNArgs(2),
	Run: func(cmd *cobra.Command, args []string) {
		local := args[1]
		remote := ""
		if len(args) > 2 {
			remote = args[2]
		}

		recursive, _ := cmd.Flags().GetBool("recursive")
		maxWorkers, _ := cmd.Flags().GetInt("max-workers")
		concurrent, _ := cmd.Flags().GetInt("concurrent-uploads")
		sshcConf := cmnflags.GetSshClientConf(cmd, args[0])
		// sshcConf.Quiet = true
		conn := sshc.NewSshConnection(sshcConf)
		go conn.Start()

		sftpConn := sshc.NewSftpClient(conn)
		go sftpConn.Start()

		if recursive {
			err := putFileRecursive(sftpConn, remote, local, maxWorkers, concurrent)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		} else {
			err := sftpConn.PutFile(remote, local, maxWorkers, &putProgressFunc)
			if err != nil {
				log.Printf("error while copying file: %s", err)
			}
		}

		putProgress.Wait()
	},
}
