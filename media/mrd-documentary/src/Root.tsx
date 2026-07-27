import './index.css';
import React from 'react';
import {Composition, Folder, Still} from 'remotion';
import {MoodTest, MoodTestPoster, type MoodTestProps} from './documentary/MoodTest';
import {Placeholder} from './documentary/Placeholder';

const moodProps = {quality: 'review', withAudio: true} satisfies MoodTestProps;

const Main4K = () => <Placeholder label="Compact Matching for Minimum Rectangular Dissection" />;
const Main1080 = () => <Placeholder label="Compact Matching for Minimum Rectangular Dissection" />;
const Teaser60 = () => <Placeholder label="MRD / 60 SECOND CUT" />;
const Teaser20 = () => <Placeholder label="MRD / 20 SECOND CUT" />;
const SilentLoop = () => <Placeholder label="MRD / SILENT LOOP" />;
const Thumbnail = () => <MoodTestPoster />;

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Folder name="V0-Review">
        <Composition
          id="MRDMoodTest1080"
          component={MoodTest}
          durationInFrames={510}
          fps={30}
          width={1920}
          height={1080}
          defaultProps={moodProps}
        />
        <Still
          id="MRDMoodTestStill"
          component={Thumbnail}
          width={1920}
          height={1080}
        />
      </Folder>
      <Folder name="Masters">
        <Composition id="MRDDocumentary4K" component={Main4K} durationInFrames={8640} fps={30} width={3840} height={2160} />
        <Composition id="MRDDocumentary1080" component={Main1080} durationInFrames={8640} fps={30} width={1920} height={1080} />
        <Composition id="MRDTeaser60" component={Teaser60} durationInFrames={1800} fps={30} width={1920} height={1080} />
        <Composition id="MRDTeaser20" component={Teaser20} durationInFrames={600} fps={30} width={1920} height={1080} />
        <Composition id="MRDSilentLoop" component={SilentLoop} durationInFrames={600} fps={30} width={1920} height={1080} />
        <Still id="MRDThumbnail" component={Thumbnail} width={3840} height={2160} />
      </Folder>
    </>
  );
};
